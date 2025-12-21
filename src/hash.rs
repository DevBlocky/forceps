/// Type definition for an array of bytes that make up an 16-byte hash (e.g., md5, xx3, etc.).
pub type HashBytes = [u8; 16];

#[inline]
pub fn create(data: &[u8]) -> HashBytes {
    #[cfg(feature = "xxhash")]
    return twox_hash::XxHash3_128::oneshot(data).to_be_bytes();
    #[cfg(not(feature = "xxhash"))]
    return md5::compute(data).0;
}
