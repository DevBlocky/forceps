# Forceps

Forceps is a fast, asynchronous, and simple cache/database built for large-file storage for HTTP
servers and other network applications. This crate is intended to be used with the `tokio`
runtime.

The motive behind this project is that there wasn't any good caching solutions for rust crates
that were publically available. The best I found was zkat/cacache-rs, however it lacked in speed
and wasn't what I was looking for exactly. This cache was specifically designed for
[scalpel](https://github.com/blockba5her/scalpel), which is an image cache server for MangaDex.

## Instability Warning

Just as a **warning**, this crate is still yet to be heavily tested and is still lacking features.
It is advisable to use another solution if you have the option!

## Features

- Asynchronous APIs
- Fast and reliable reading/writing
- Tuned for large-file databases
- Easily accessible value metadata
- Optimized for cache `HIT`s
- Easy error handling

### Features-to-come

- Toggleable in-memory LRU cache
- Optional last-access timestamps
- Removing database entries
- Easy cache eviction

## Documentation

All documentation for this project can be found at [docs.rs](https://docs.rs/forceps/*/forceps).

## License

This project is licensed under the `MIT` license. Please see
[LICENSE](https://github.com/blockba5her/forceps/blob/main/LICENSE) for more information.
