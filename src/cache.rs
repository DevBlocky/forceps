use crate::{ForcepError, MetaDb, Metadata, Result};
use std::io;
use std::path;
use tokio::fs as afs;

/// Creates a writeable and persistent temporary file in the path provided, returning the path and
/// file handle.
async fn tempfile(dir: &path::Path) -> Result<(afs::File, path::PathBuf)> {
    let tmppath = crate::tmp::tmppath_in(dir);
    let tmp = afs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmppath)
        .await
        .map_err(ForcepError::Io)?;
    Ok((tmp, tmppath))
}

/// The main component of `forceps`, acts as the API for interacting with the on-disk API.
///
/// This structure exposes `read`, `write`, and misc metadata operations. `read` and `write` are
/// both async, whereas all metadata operations are sync. To create this structure, use the
/// [`CacheBuilder`].
///
/// # Examples
///
/// ```rust
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() {
/// use forceps::CacheBuilder;
///
/// let cache = CacheBuilder::new("./cache")
///     .build()
///     .await
///     .unwrap();
/// # }
/// ```
#[derive(Debug)]
pub struct Cache {
    meta: MetaDb,
    path: path::PathBuf,
}

/// A builder for the [`Cache`] object. Exposes APIs for configuring the initial setup of the
/// database.
///
/// # Examples
///
/// ```rust
/// use forceps::CacheBuilder;
///
/// let builder = CacheBuilder::new("./cache");
/// ```
#[derive(Debug, Clone)]
pub struct CacheBuilder {
    path: path::PathBuf,
}

impl Cache {
    /// Creates a new Cache instance based on the CacheBuilder
    async fn new(builder: CacheBuilder) -> Result<Self> {
        // create the base directory for the cache
        afs::create_dir_all(&builder.path)
            .await
            .map_err(ForcepError::Io)?;

        let mut meta_path = builder.path.clone();
        meta_path.push("index");
        Ok(Self {
            meta: MetaDb::new(&meta_path)?,
            path: builder.path,
        })
    }

    /// Creates a PathBuf based on the key provided
    fn path_from_key(&self, key: &[u8]) -> path::PathBuf {
        let hex = hex::encode(key);
        let mut buf = self.path.clone();

        // push segments of key as paths to the PathBuf. If the hex isn't long enough, then push
        // "__" instead.
        for n in (0..4usize).step_by(2) {
            let n_end = n + 2;
            buf.push(if n_end >= hex.len() {
                "__"
            } else {
                &hex[n..n_end]
            })
        }
        buf.push(&hex);
        buf
    }

    /// Reads an entry from the database, returning a vector of bytes that represent the entry.
    ///
    /// # Not Found
    ///
    /// If the entry is not found, then it will return
    /// `Err(`[`Error::NotFound`](ForcepError::NotFound)`)`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() {
    /// use forceps::CacheBuilder;
    ///
    /// let cache = CacheBuilder::new("./cache")
    ///     .build()
    ///     .await
    ///     .unwrap();
    /// # cache.write(b"MY_KEY", b"Hello World").await.unwrap();
    ///
    /// let value = cache.read(b"MY_KEY").await.unwrap();
    /// assert_eq!(&value, b"Hello World");
    /// # }
    /// ```
    pub async fn read<K: AsRef<[u8]>>(&self, key: K) -> Result<Vec<u8>> {
        use tokio::io::AsyncReadExt;

        let file = {
            let path = self.path_from_key(key.as_ref());
            afs::OpenOptions::new()
                .read(true)
                .open(&path)
                .await
                .map_err(|e| match e.kind() {
                    io::ErrorKind::NotFound => ForcepError::NotFound,
                    _ => ForcepError::Io(e),
                })?
        };

        // create a new buffer based on the estimated size of the file
        let size_guess = file.metadata().await.map(|x| x.len()).unwrap_or(0);
        let mut buf = Vec::with_capacity(size_guess as usize);

        // read the entire file to the buffer
        tokio::io::BufReader::new(file)
            .read_to_end(&mut buf)
            .await
            .map_err(ForcepError::Io)?;
        Ok(buf)
    }

    /// Writes an entry with the specified key to the cache database. This will replace the
    /// previous entry if it exists, otherwise it will store a completely new one.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() {
    /// use forceps::CacheBuilder;
    ///
    /// let cache = CacheBuilder::new("./cache")
    ///     .build()
    ///     .await
    ///     .unwrap();
    ///
    /// cache.write(b"MY_KEY", b"Hello World").await.unwrap();
    /// # }
    /// ```
    pub async fn write<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        let key = key.as_ref();
        let value = value.as_ref();

        let (tmp, tmp_path) = tempfile(&self.path).await?;
        // write all data to a temporary file
        {
            let mut writer = tokio::io::BufWriter::new(tmp);
            writer.write_all(value).await.map_err(ForcepError::Io)?;
            writer.flush().await.map_err(ForcepError::Io)?;
        }

        // move the temporary file to the final destination
        let final_path = self.path_from_key(key);
        if let Some(parent) = final_path.parent() {
            afs::create_dir_all(parent).await.map_err(ForcepError::Io)?;
        }
        afs::rename(&tmp_path, &final_path)
            .await
            .map_err(ForcepError::Io)?;

        self.meta.insert_metadata_for(key, value)?;

        Ok(())
    }

    /// Queries the index database for metadata on the entry with the corresponding key.
    ///
    /// This will return the metadata for the associated key. For information about what metadata
    /// is stored, look at [`Metadata`].
    ///
    /// # Non-Async
    ///
    /// Note that this function is not an async call. This is because the backend database used,
    /// `sled`, is not async-compatible. However, these calls are instead very fast.
    ///
    /// # Not Found
    ///
    /// If the entry is not found, then it will return
    /// `Err(`[`Error::NotFound`](ForcepError::NotFound)`)`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() {
    /// use forceps::CacheBuilder;
    ///
    /// let cache = CacheBuilder::new("./cache")
    ///     .build()
    ///     .await
    ///     .unwrap();
    ///
    /// # cache.write(b"MY_KEY", b"Hello World").await.unwrap();
    /// let meta = cache.read_metadata(b"MY_KEY").unwrap();
    /// assert_eq!(meta.get_size(), b"Hello World".len() as u64);
    /// # }
    /// ```
    pub fn read_metadata<K: AsRef<[u8]>>(&self, key: K) -> Result<Metadata> {
        self.meta.get_metadata(key.as_ref())
    }

    /// An iterator over the entire metadata database, which provides metadata for every entry.
    ///
    /// This iterator provides every key in the database and the associated metadata for that key.
    /// This is *not* an iterator over the actual values of the database.
    ///
    /// # Non-Async
    ///
    /// Note that this function is not an async call. This is because the backend database used,
    /// `sled`, is not async-compatible. However, these calls are instead very fast.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() {
    /// use forceps::CacheBuilder;
    ///
    /// let cache = CacheBuilder::new("./cache")
    ///     .build()
    ///     .await
    ///     .unwrap();
    ///
    /// # cache.write(b"MY_KEY", b"Hello World").await.unwrap();
    /// for result in cache.metadata_iter() {
    ///     let (key, meta) = result.unwrap();
    ///     println!("{}", String::from_utf8_lossy(&key))
    /// }
    /// # }
    /// ```
    pub fn metadata_iter(&self) -> impl Iterator<Item = Result<(Vec<u8>, Metadata)>> {
        self.meta.metadata_iter()
    }
}

impl CacheBuilder {
    /// Creates a new [`CacheBuilder`], which can be used to customize and create a [`Cache`]
    /// instance.
    ///
    /// The `path` supplied is the base directory of the cache instance.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use forceps::CacheBuilder;
    ///
    /// let builder = CacheBuilder::new("./cache");
    /// // Use other methods for configuration
    /// ```
    pub fn new<P: AsRef<path::Path>>(path: P) -> Self {
        CacheBuilder {
            path: path.as_ref().to_owned(),
        }
    }

    /// Builds the new [`Cache`] instance using the configured options of the builder.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() {
    /// use forceps::CacheBuilder;
    ///
    /// let cache = CacheBuilder::new("./cache")
    ///     .build()
    ///     .await
    ///     .unwrap();
    /// # }
    /// ```
    pub async fn build(self) -> Result<Cache> {
        Cache::new(self).await
    }
}

impl Default for CacheBuilder {
    /// Creates a [`CacheBuilder`] with the directory set to `./cache`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() {
    /// use forceps::CacheBuilder;
    ///
    /// let cache = CacheBuilder::default()
    ///     .build()
    ///     .await
    ///     .unwrap();
    /// # }
    /// ```
    fn default() -> Self {
        const DIR: &str = "./cache";
        Self::new(DIR)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    async fn default_cache() -> Cache {
        CacheBuilder::default().build().await.unwrap()
    }

    #[tokio::test]
    async fn short_path() {
        let cache = default_cache().await;
        cache.path_from_key(&[0xAA]);
        cache.path_from_key(&[0xAA, 0xBB]);
        cache.path_from_key(&[0xAA, 0xBB, 0xCC]);
    }

    #[tokio::test]
    async fn basic_write_read() {
        let cache = default_cache().await;

        cache.write(&b"CACHE_KEY", &b"Hello World").await.unwrap();
        let data = cache.read(&b"CACHE_KEY").await.unwrap();
        assert_eq!(&data, &b"Hello World");
    }

    #[tokio::test]
    async fn read_metadata() {
        let cache = default_cache().await;

        cache.write(&b"CACHE_KEY", &b"Hello World").await.unwrap();
        let metadata = cache.read_metadata(&b"CACHE_KEY").unwrap();
        assert_eq!(metadata.get_size(), b"Hello World".len() as u64);
    }
}
