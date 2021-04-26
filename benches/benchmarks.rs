use criterion::{criterion_group, Criterion};
use rand::prelude::*;
use std::time::Instant;

fn make_executor() -> (forceps::Cache, tokio::runtime::Runtime) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let cache = rt.block_on(async move { forceps::CacheBuilder::default().build().await.unwrap() });
    (cache, rt)
}

fn random_bytes(size: usize) -> Vec<u8> {
    let mut buf = vec![0u8; size];
    let mut rng = rand::rngs::OsRng::default();
    rng.fill_bytes(&mut buf);
    buf
}

/// A value size that simulates a regular workload for the cache
/// Current is 600KiB
const VALUE_SZ: usize = 1024 * 600;

pub fn cache_write_const_key(c: &mut Criterion) {
    c.bench_function("cache::write_const_key", move |b| {
        let (db, rt) = make_executor();
        const KEY: [u8; 4] = [0xDE, 0xAD, 0xBE, 0xEF];
        let value = random_bytes(VALUE_SZ);

        b.iter(|| {
            rt.block_on(db.write(&KEY, &value)).unwrap();
        });
    });
}

pub fn cache_write_random_key(c: &mut Criterion) {
    c.bench_function("cache::write_random_key", move |b| {
        let (db, rt) = make_executor();
        let value = random_bytes(VALUE_SZ);

        b.iter_custom(|iters| {
            let key = random_bytes(4);
            let start = Instant::now();
            for _ in 0..iters {
                rt.block_on(db.write(&key, &value)).unwrap();
            }
            start.elapsed()
        });
    });
}

pub fn cache_read_const_key(c: &mut Criterion) {
    c.bench_function("cache::read_const_key", move |b| {
        let (db, rt) = make_executor();
        const KEY: [u8; 4] = [0xDE, 0xAD, 0xBE, 0xEF];
        let value = random_bytes(VALUE_SZ);

        // assert there is the key in the db
        rt.block_on(db.write(&KEY, &value)).unwrap();

        b.iter(|| {
            rt.block_on(db.read(&KEY)).unwrap();
        });
    });
}

pub fn cache_metadata_lookup(c: &mut Criterion) {
    c.bench_function("cache::metadata_lookup", move |b| {
        let (db, rt) = make_executor();
        const KEY: [u8; 4] = [0xDE, 0xAD, 0xBE, 0xEF];
        let value = random_bytes(VALUE_SZ);
        rt.block_on(db.write(&KEY, &value)).unwrap();

        b.iter(|| {
            db.read_metadata(&KEY).unwrap();
        });
    });
}

criterion_group!(
    benches,
    cache_write_const_key,
    cache_write_random_key,
    cache_read_const_key,
    cache_metadata_lookup
);

fn main() {
    std::fs::remove_dir_all("./cache").unwrap();

    benches();

    Criterion::default().configure_from_args().final_summary();
}
