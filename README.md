[![license](https://img.shields.io/github/license/RedisLabsModules/redismodule-rs.svg)](https://github.com/valkey-io/valkeymodule-rs/blob/main/LICENSE)
[![Releases](https://img.shields.io/github/release/RedisLabsModules/redismodule-rs.svg)](https://github.com/valkey-io/valkeymodule-rs/releases)
[![crates.io](https://img.shields.io/crates/v/redis-module.svg)](https://crates.io/crates/valkey-module)
[![docs](https://docs.rs/redis-module/badge.svg)](https://docs.rs/valkey-module)
[![CircleCI](https://circleci.com/gh/RedisLabsModules/redismodule-rs/tree/master.svg?style=svg)](https://circleci.com/gh/RedisLabsModules/redismodule-rs/tree/master)

# valkeymodule-rs

This crate provides an idiomatic Rust API for the [Valkey Modules API](https://valkey.io/topics/modules-api-ref/).
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
Therefore, following the [Valkey Modules API](https://valkey.io/topics/modules-api-ref/) documentation will be mostly relevant here as well.

## Feature Flags

### System Allocator

This feature flag is ideal for unit testing where the engine server is not running, and we do not have access to the Valkey engine allocator, so we can use the system allocator instead.
To optionally enter the `System.alloc` code paths in `alloc.rs` specify this in `Cargo.toml` of your module:
```toml
[features]
enable-system-alloc = ["valkey-module/enable-system-alloc"]
```
For unit tests with `System.alloc` use this:
```sh
cargo test --features enable-system-alloc
```
For integration tests with `ValkeyAlloc` use this:
```sh
cargo test
```

### Testing `ValkeyString` without a Valkey server

The `test-shims` feature provides `ValkeyString::test()`, which creates a binary-safe `ValkeyString` for unit tests without starting a Valkey process. Add `valkey-module` with this feature to your development dependencies, using the same version or source as your normal dependency:

```toml
[dev-dependencies]
valkey-module = { version = "...", features = ["test-shims"] }
```

The feature automatically enables the system allocator required by tests. It is available only to development and test builds when configured as a dev dependency.

`ValkeyString::test()` accepts any value implementing `Into<Vec<u8>>`, including strings and arbitrary bytes:

```rust
#[cfg(test)]
mod tests {
    use valkey_module::ValkeyString;

    #[test]
    fn creates_strings_without_valkey() {
        let text = ValkeyString::test("hello");
        let binary = ValkeyString::test(vec![0x00, 0xff]);

        assert_eq!(text.as_slice(), b"hello");
        assert_eq!(binary.as_slice(), &[0x00, 0xff]);
    }
}
```

Run these tests normally:

```sh
cargo test
```

The first call installs process-wide test callbacks for string reading, cloning, comparison, numeric parsing, retaining, and freeing. Do not enable or invoke the test shims in code running inside a Valkey process. The setup rejects installation after the real Valkey API has been initialized. APIs without a shim, such as `ValkeyString::create()` and `ValkeyString::append()`, still require a running Valkey server.

### Redis Compatibility

This feature flag is useful in case you have a Module that needs to be loaded on both Valkey and Redis Servers. In this case, you can use the `use-redismodule-api` flag so that the Module is loaded using the RedisModule API Initialization for compatibility.

To use this feature by conditionally, specify the following in your `Cargo.toml`:
```toml
[features]
use-redismodule-api = ["valkey-module/use-redismodule-api"]
default = []
```

```sh
cargo build --release --features use-redismodule-api
```
