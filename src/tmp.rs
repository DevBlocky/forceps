use rand::{distr::Alphanumeric, prelude::*};
use std::path;
use std::str;

fn tmpname(prefix: &str, rand_len: usize) -> String {
    let mut buf = String::with_capacity(prefix.len() + rand_len);
    buf.push_str(prefix);
    // push random characters one-by-one
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(rand_len)
        .for_each(|b| buf.push(b as char));
    buf
}

/// Creates a randomized path in a directory that can be used as a temporary file
pub(crate) fn tmppath_in(dir: &path::Path) -> path::PathBuf {
    const LEN: usize = 10;
    let mut buf = path::PathBuf::new();
    buf.push(dir);
    buf.push(tmpname("tmp", LEN));
    buf
}
