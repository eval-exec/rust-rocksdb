rust-rocksdb
============
[![Build Status](https://github.com/nervosnetwork/rust-rocksdb/actions/workflows/rust.yml/badge.svg?branch=txn)](https://github.com/nervosnetwork/rust-rocksdb/actions/workflows/rust.yml?query=branch%3Atxn)
[![crates.io](https://img.shields.io/crates/v/ckb-rocksdb.svg)](https://crates.io/crates/ckb-rocksdb)
[![documentation](https://docs.rs/ckb-rocksdb/badge.svg)](https://docs.rs/ckb-rocksdb)
[![license](https://img.shields.io/crates/l/ckb-rocksdb.svg)](https://github.com/nervosnetwork/rust-rocksdb/blob/txn/LICENSE)
[![Discord](https://img.shields.io/badge/chat-on%20Discord-7289DA.svg)](https://discord.com/invite/nervos)


## Requirements

- Clang and LLVM

## Contributing

Feedback and pull requests welcome!  If a particular feature of RocksDB is
important to you, please let me know by opening an issue, and I'll
prioritize it.

## Usage

This binding is statically linked with a specific version of RocksDB. If you
want to build it yourself, make sure you've also cloned the RocksDB and
compression submodules:

    git submodule update --init --recursive

## Compression Support
By default, support for the [Snappy](https://github.com/google/snappy),
[LZ4](https://github.com/lz4/lz4), [Zstd](https://github.com/facebook/zstd),
[Zlib](https://zlib.net), and [Bzip2](http://www.bzip.org) compression
is enabled through crate features.  If support for all of these compression
algorithms is not needed, default features can be disabled and specific
compression algorithms can be enabled. For example, to enable only LZ4
compression support, make these changes to your Cargo.toml:

```
[dependencies.rocksdb]
default-features = false
features = ["lz4"]
```
