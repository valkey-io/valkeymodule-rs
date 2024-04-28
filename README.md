[![license](https://img.shields.io/github/license/RedisLabsModules/redismodule-rs.svg)](https://github.com/RedisLabsModules/redismodule-rs/blob/master/LICENSE)
[![Releases](https://img.shields.io/github/release/RedisLabsModules/redismodule-rs.svg)](https://github.com/RedisLabsModules/redismodule-rs/releases/latest)
[![crates.io](https://img.shields.io/crates/v/redis-module.svg)](https://crates.io/crates/redis-module)
[![docs](https://docs.rs/redis-module/badge.svg)](https://docs.rs/redis-module)
[![CircleCI](https://circleci.com/gh/RedisLabsModules/redismodule-rs/tree/master.svg?style=svg)](https://circleci.com/gh/RedisLabsModules/redismodule-rs/tree/master)

# valkeymodule-rs

This crate provides an idiomatic Rust API for the [Valkey Modules API](https://redis.io/topics/modules-intro).
It allows writing Valkey modules in Rust, without needing to use raw pointers or unsafe code. See [here](https://docs.rs/valkey-module/latest) for the most recent API documentation.

This repo was forked from [redismodule-rs](https://github.com/RedisLabsModules/redismodule-rs).  We appreciate the contributions of the original authors.  

# Running the example module

1. [Install Rust](https://www.rust-lang.org/tools/install)
2. [Install Valkey](https://valkey.io/download/), most likely using your favorite package manager (Homebrew on Mac, APT or YUM on Linux)
3. Run `cargo build --example hello`
4. Start a valkey server with the `hello` module
   * Linux: `valkey-server --loadmodule ./target/debug/examples/libhello.so`
   * Mac: `valkey-server --loadmodule ./target/debug/examples/libhello.dylib`
5. Open a valkey-cli, and run `HELLO.MUL 31 11`.

# Writing your own module

See the [examples](examples) directory for some sample modules.

This crate tries to provide high-level wrappers around the standard Valkey Modules API, while preserving the API's basic concepts.
Therefore, following the [Valkeyi Modules API](https://redis.io/topics/modules-intro) documentation will be mostly relevant here as well.
