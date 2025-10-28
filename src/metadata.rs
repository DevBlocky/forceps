use crate::{ForcepError, Result};
use std::path;
use std::time;

/// Type definition for an array of bytes that make up an `md5` hash.
pub type Md5Bytes = [u8; 16];

/// Metadata information about a certain entry in the cache
///
/// This metadata contains information about when the entry was last modified, the size (in bytes)
/// of the entry, the `md5` integrity of the entry, etc.
///
/// # Examples
///
/// ```rust
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() {
/// use forceps::Cache;
///
/// let cache = Cache::new("./cache")
///     .build()
///     .await
///     .unwrap();
///
/// cache.write(&b"MY_KEY", &b"Hello World").await.unwrap();
///
/// let metadata = cache.read_metadata(&b"MY_KEY").unwrap();
/// # }
/// ```
#[derive(Debug)]
pub struct Metadata {
    /// Size in bytes of the corresponding entry
    size: u64,
    /// Last time this entry was modified, milliseconds since epoch
    last_modified: u64,
    /// Last time since this entry was accessed, milliseconds since epoch
    last_accessed: u64,
    /// Number of times this entry has been HIT (total accesses)
    hits: u64,
    /// Md5 hash of the underlying data
    integrity: Md5Bytes,
}

/// Milliseconds from epoch to now
fn now_since_epoch() -> u64 {
    time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .map(|x| x.as_millis() as u64)
        .unwrap_or(0)
}

impl Metadata {
    /// Creates a new instance of [`Metadata`] from the given `data`
    pub(crate) fn new(data: &[u8]) -> Self {
        Self {
            size: data.len() as u64,
            last_modified: now_since_epoch(),
            last_accessed: now_since_epoch(),
            hits: 0,
            integrity: md5::compute(data).into(),
        }
    }

    /// Serializes the metadata into bytes
    pub(crate) fn serialize(&self) -> Vec<u8> {
        use bson::{
            cstr,
            raw::{RawBinaryRef, RawBson, RawDocumentBuf},
        };

        let mut doc = RawDocumentBuf::new();
        doc.append(cstr!("size"), RawBson::Int64(self.size as i64));
        doc.append(
            cstr!("last_modified"),
            RawBson::Int64(self.last_modified as i64),
        );
        doc.append(
            cstr!("last_accessed"),
            RawBson::Int64(self.last_accessed as i64),
        );
        doc.append(cstr!("hits"), RawBson::Int64(self.hits as i64));
        doc.append(
            cstr!("integrity"),
            RawBinaryRef {
                subtype: bson::spec::BinarySubtype::Md5,
                bytes: &self.integrity,
            },
        );
        doc.into_bytes()
    }

    /// Deserializes a slice of bytes into metadata
    pub(crate) fn deserialize(buf: &[u8]) -> Result<Self> {
        use bson::{error::Error as BsonError, raw::RawDocument, spec::BinarySubtype};

        let doc = RawDocument::from_bytes(buf).map_err(ForcepError::MetaDe)?;

        let make_error = |key: &str, msg: &str| -> ForcepError {
            let io_err = std::io::Error::new(std::io::ErrorKind::InvalidData, msg.to_owned());
            let mut err = BsonError::from(io_err);
            err.key = Some(key.to_owned());
            ForcepError::MetaDe(err)
        };

        let read_u64 = |key: &str| -> Result<u64> {
            doc.get_i64(key)
                .map(|v| v as u64)
                .map_err(ForcepError::MetaDe)
        };

        let size = read_u64("size")?;
        let last_modified = read_u64("last_modified")?;
        let last_accessed = read_u64("last_accessed")?;
        let hits = read_u64("hits")?;

        let binary = doc.get_binary("integrity").map_err(ForcepError::MetaDe)?;
        if binary.subtype != BinarySubtype::Md5 {
            return Err(make_error("integrity", "expected MD5 binary subtype"));
        }
        const MD5_LEN: usize = 16;
        if binary.bytes.len() != MD5_LEN {
            return Err(make_error("integrity", "integrity must contain 16 bytes"));
        }
        let mut integrity = [0u8; MD5_LEN];
        integrity.copy_from_slice(binary.bytes);

        Ok(Self {
            size,
            last_modified,
            last_accessed,
            hits,
            integrity,
        })
    }

    /// The size in bytes of the corresponding cache entry.
    #[inline]
    pub fn get_size(&self) -> u64 {
        self.size
    }

    /// Retrives the last time this entry was modified.
    #[inline]
    pub fn get_last_modified(&self) -> Option<time::SystemTime> {
        match self.last_modified {
            0 => None,
            millis => Some(time::UNIX_EPOCH + time::Duration::from_millis(millis)),
        }
    }
    /// Retrieves the raw `last_modified` time, which is the milliseconds since
    /// [`time::UNIX_EPOCH`]. If the returned result is `0`, that means there is no `last_modified`
    /// time.
    #[inline]
    pub fn get_last_modified_raw(&self) -> u64 {
        self.last_modified
    }

    /// The total number of times this entry has been read.
    ///
    /// **NOTE:** This will be 0 unless `track_access` is enabled from the [`CacheBuilder`]
    ///
    /// [`CacheBuilder`]: crate::CacheBuilder
    #[inline]
    pub fn get_hits(&self) -> u64 {
        self.hits
    }

    /// Retrives the last time this entry was accessed (read from).
    ///
    /// **NOTE:** This will be the same as [`get_last_modified`] unless `track_access` is enabled from
    /// the [`CacheBuilder`]
    ///
    /// [`get_last_modified`]: Self::get_last_modified
    /// [`CacheBuilder`]: crate::CacheBuilder
    #[inline]
    pub fn get_last_acccessed(&self) -> Option<time::SystemTime> {
        match self.last_accessed {
            0 => None,
            millis => Some(time::UNIX_EPOCH + time::Duration::from_millis(millis)),
        }
    }
    /// Retrieves the raw `last_accessed` time, which is the milliseconds since
    /// [`time::UNIX_EPOCH`]. If the returned result is `0`, that means there is no `last_accessed`
    /// time.
    ///
    /// **NOTE:** This will be the same as [`get_last_modified_raw`] unless `track_access` is enabled
    /// from the [`CacheBuilder`]
    ///
    /// [`get_last_modified_raw`]: Self::get_last_modified_raw
    /// [`CacheBuilder`]: crate::CacheBuilder
    #[inline]
    pub fn get_last_accessed_raw(&self) -> u64 {
        self.last_accessed
    }

    /// Retrieves the internal [`Md5Bytes`] integrity of the corresponding metadata entry.
    #[inline]
    pub fn get_integrity(&self) -> &Md5Bytes {
        &self.integrity
    }

    /// Verifies that the metadata integrity matches the integrity of the data provided.
    #[inline]
    pub fn check_integrity_of(&self, data: &[u8]) -> bool {
        let other_integrity: Md5Bytes = md5::compute(data).into();
        other_integrity == self.integrity
    }
}

/// Database for cache entry metadata
pub(crate) struct MetaDb {
    db: feoxdb::FeoxStore,
}

impl MetaDb {
    /// Initializes a new metadata database with sled.
    pub fn new(path: &path::Path) -> Result<Self> {
        let db = feoxdb::FeoxStore::builder()
            .device_path(path.to_string_lossy())
            .build()
            .map_err(ForcepError::MetaDb)?;
        Ok(Self { db })
    }

    /// Retrieves an entry in the metadata database with the corresponding key.
    pub fn get_metadata(&self, key: &[u8]) -> Result<Metadata> {
        let data = match self.db.get_bytes(key) {
            Ok(data) => data,
            Err(feoxdb::FeoxError::KeyNotFound) => return Err(ForcepError::MetaNotFound),
            Err(e) => return Err(ForcepError::MetaDb(e)),
        };
        Metadata::deserialize(&data)
    }

    /// Inserts a new entry into the metadata database for the associated key and data.
    ///
    /// If a previous entry exists, it is simply overwritten.
    pub fn insert_metadata_for(&self, key: &[u8], data: &[u8]) -> Result<Metadata> {
        let meta = Metadata::new(data);
        let bytes = Metadata::serialize(&meta);
        self.db
            .insert(key, &bytes[..])
            .map_err(ForcepError::MetaDb)?;
        Ok(meta)
    }

    pub fn remove_metadata_for(&self, key: &[u8]) -> Result<Metadata> {
        let meta = match self.db.get_bytes(key) {
            Ok(data) => Metadata::deserialize(&data)?,
            Err(feoxdb::FeoxError::KeyNotFound) => return Err(ForcepError::MetaNotFound),
            Err(e) => return Err(ForcepError::MetaDb(e)),
        };
        self.db.delete(key).map_err(ForcepError::MetaDb)?;
        Ok(meta)
    }

    /// Will increment the `hits` counter and set the `last_accessed` value to now for the found
    /// metadata key.
    pub fn track_access_for(&self, key: &[u8]) -> Result<Metadata> {
        let mut meta = match self.db.get_bytes(key) {
            Ok(data) => Metadata::deserialize(&data)?,
            Err(feoxdb::FeoxError::KeyNotFound) => return Err(ForcepError::MetaNotFound),
            Err(e) => return Err(ForcepError::MetaDb(e)),
        };
        meta.last_accessed = now_since_epoch();
        meta.hits += 1;
        self.db
            .insert(key, &Metadata::serialize(&meta))
            .map_err(ForcepError::MetaDb)?;
        Ok(meta)
    }

    /// Iterator over the entire metadata database
    pub fn metadata_iter(&self) -> impl Iterator<Item = Result<(Vec<u8>, Metadata)>> {
        vec![].into_iter()
        // self.db.iter().map(|x| match x {
        //     Ok((key, data)) => Metadata::deserialize(&data[..]).map(|m| (key.to_vec(), m)),
        //     Err(e) => Err(ForcepError::MetaDb(e)),
        // })
    }
}

impl std::fmt::Debug for MetaDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "FeOxDb metadata database")
    }
}

#[cfg(test)]
mod test {
    use super::*;
    const DATA: [u8; 4] = [0xDE, 0xAD, 0xBE, 0xEF];

    fn create_db() -> Result<MetaDb> {
        const META_TESTDIR: &str = "./cache/test-index";
        let path = path::PathBuf::from(META_TESTDIR);
        MetaDb::new(&path)
    }

    #[test]
    fn create_metadb() {
        create_db().unwrap();
    }

    #[test]
    fn db_read_write() {
        let db = create_db().unwrap();
        db.insert_metadata_for(&DATA, &DATA).unwrap();
        let meta = db.get_metadata(&DATA).unwrap();
        assert_eq!(meta.get_size(), DATA.len() as u64);
    }

    #[test]
    fn check_integrity() {
        let db = create_db().unwrap();
        let meta = db.insert_metadata_for(&DATA, &DATA).unwrap();
        assert!(meta.check_integrity_of(&DATA));
    }

    #[test]
    fn last_modified() {
        let db = create_db().unwrap();
        let meta = db.insert_metadata_for(&DATA, &DATA).unwrap();
        // make sure last-modified date is within last second
        assert_eq!(
            meta.get_last_modified()
                .unwrap()
                .elapsed()
                .unwrap()
                .as_secs(),
            0
        );
    }

    #[test]
    fn metadata_ser_de() {
        let db = create_db().unwrap();
        let meta = db.insert_metadata_for(&DATA, &DATA).unwrap();
        let ser_bytes = meta.serialize();
        let de = Metadata::deserialize(&ser_bytes).unwrap();
        assert_eq!(meta.get_integrity(), de.get_integrity());
    }
}
