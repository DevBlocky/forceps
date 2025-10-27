use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rand::prelude::*;

const CACHE_DIRECTORY: &str = "cache";

fn make_executor_custom<F: FnOnce() -> forceps::CacheBuilder>(
    f: F,
) -> (forceps::Cache, tokio::runtime::Runtime) {
    use std::{fs, io};

    match fs::remove_dir_all(CACHE_DIRECTORY) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => panic!("{e}"),
        _ => {}
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let cache = rt.block_on(async move { f().build().await.unwrap() });
    (cache, rt)
}
fn make_executor() -> (forceps::Cache, tokio::runtime::Runtime) {
    make_executor_custom(|| forceps::CacheBuilder::new(CACHE_DIRECTORY))
}

fn random_bytes(size: usize) -> Vec<u8> {
    let mut buf = vec![0u8; size];
    rand::rng().fill_bytes(&mut buf);
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

        b.iter_with_setup(
            || random_bytes(4),
            |key| {
                rt.block_on(db.write(&key, &value)).unwrap();
            },
        );
    });
}

pub fn cache_read_const_key(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache::read_const_key");
    for &tracking in [false, true].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(if tracking { "tracked" } else { "untracked" }),
            &tracking,
            move |b, &tracking| {
                let (db, rt) = make_executor_custom(|| {
                    forceps::CacheBuilder::new(CACHE_DIRECTORY).track_access(tracking)
                });
                const KEY: [u8; 4] = [0xDE, 0xAD, 0xBE, 0xEF];
                let value = random_bytes(VALUE_SZ);

                // assert there is the key in the db
                rt.block_on(db.write(&KEY, &value)).unwrap();

                b.iter(|| {
                    rt.block_on(db.read(&KEY)).unwrap();
                });
            },
        );
    }
}

pub fn cache_remove_const_key(c: &mut Criterion) {
    c.bench_function("cache::remove_const_key", move |b| {
        std::fs::remove_dir_all("./cache").unwrap();
        let (db, rt) = make_executor();
        const KEY: [u8; 4] = [0xDE, 0xAD, 0xBE, 0xEF];
        let value = random_bytes(VALUE_SZ);

        b.iter_with_setup(
            || rt.block_on(db.write(&KEY, &value)).unwrap(),
            |_| {
                rt.block_on(db.remove(&KEY)).unwrap();
            },
        );
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

fn bench_config() -> Criterion {
    Criterion::default()
        .measurement_time(std::time::Duration::from_secs(10))
        .configure_from_args()
}

criterion_group! {
    name = benches;
    config = bench_config();
    targets =
        cache_write_const_key,
        cache_write_random_key,
        cache_read_const_key,
        cache_remove_const_key,
        cache_metadata_lookup
}

criterion_main!(benches);
// fn main() {
//     // delete cache directory if it exists
//     // this is to make sure we're benching on a clean slate
//     if let Ok(_) = std::fs::read_dir("./cache") {
//         std::fs::remove_dir_all("./cache").unwrap();
//     }

//     benches();

//     Criterion::default().configure_from_args().final_summary();
// }
