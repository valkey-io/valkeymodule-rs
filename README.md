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

### Unit testing without a Valkey server

The `test-shims` feature provides `Context::test()`, `ThreadSafeContext::test()`, `CommandFilterCtx::test()`, `InfoContext::test()`, and `ValkeyString::test()` for unit tests that run without starting a Valkey process. Add `valkey-module` with this feature to your development dependencies, using the same version or source as your normal dependency:

```toml
[dev-dependencies]
valkey-module = { version = "...", features = ["test-shims"] }
```

The feature automatically enables the system allocator required by tests. When configured as a development dependency, the test-only APIs are available to unit tests without enabling them in production builds.

Use `Context::test()` when a command handler needs a context. Its `expect_*` methods configure values returned by supported context APIs. Use `ValkeyString::test()` to construct binary-safe command arguments; it accepts any value implementing `Into<Vec<u8>>`, including strings and arbitrary bytes:

```rust
#[cfg(test)]
mod tests {
    use valkey_module::{Context, ValkeyString};

    #[test]
    fn uses_test_context_and_strings_without_valkey() {
        let mut context = Context::test();
        context
            .expect_get_client_id(42)
            .expect_get_server_version(8, 1, 2);

        let text = ValkeyString::test("hello");
        let binary = ValkeyString::test(vec![0x00, 0xff]);
        let version = context
            .get_server_version()
            .expect("configured server version should be returned");

        assert_eq!(context.get_client_id(), 42);
        assert_eq!((version.major, version.minor, version.patch), (8, 1, 2));
        assert_eq!(text.as_slice(), b"hello");
        assert_eq!(binary.as_slice(), &[0x00, 0xff]);
    }
}
```

Use `test_shims::create_test_args()` to convert text or binary values implementing `AsRef<[u8]>` into a `Vec<ValkeyString>`:

```rust
use valkey_module::test_shims::create_test_args;

let args = create_test_args(&["example.command", "first", "second"]);
let binary_args = create_test_args(&[b"example.command".as_slice(), b"\0\xff".as_slice()]);
```

Use `InfoContext::test()` to invoke an INFO handler directly and inspect the typed sections it emits. `sections()` returns an owned snapshot that preserves section, field, and dictionary insertion order:

```rust
use valkey_module::test_shims::{TestInfoEntry, TestInfoValue};
use valkey_module::{InfoContext, InfoContextBuilderFieldBottomLevelValue};

let info = InfoContext::test();
info.builder()
    .add_section("metrics")
    .field("status", "ready")
    .expect("status field should be unique")
    .field("ratio", InfoContextBuilderFieldBottomLevelValue::F64(0.5))
    .expect("ratio field should be unique")
    .build_section()
    .expect("section should be unique")
    .build_info()
    .expect("INFO data should build");

assert!(matches!(
    &info.sections()[0].entries[0],
    TestInfoEntry::Field(field)
        if matches!(&field.value, TestInfoValue::String(value) if value == "ready")
));
```

Call `expect_unrequested_section(name)` before invoking a handler to model Valkey declining an unrequested INFO section. The `info_handler_builder`, `info_handler_macro`, `info_handler_multiple_sections`, `info_handler_struct`, and `test_helper` examples demonstrate serverless INFO-handler unit tests.

The test context supports configuring the server version, client IDs, names, usernames, certificates, client information and IP addresses, the current user, ACL-user authentication, and client deauthentication. Configure `Context::get_server_version()` with `expect_get_server_version(major, minor, patch)`.

For client-specific expectations, use `expect_get_client_name_by_id`, `expect_get_client_username_by_id`, `expect_get_client_ip_by_id`, or `expect_set_client_name_by_id`. Configure detailed client information with `expect_get_client_info_by_id(RedisModuleClientInfo { id, ..Default::default() })`; the ID is read from the supplied structure. `expect_get_client_cert` configures the certificate returned for the current client. Each `Context::test()` resets these expectations, so tests do not inherit client state from earlier tests on the same thread.

Use `expect_call` to configure an exact `Context::call()` command and argument list. It supports simple strings, integers, booleans, doubles, big numbers, nulls, maps, and nested arrays. An unconfigured call returns an error describing the unexpected command instead of invoking Valkey:

```rust
use valkey_module::ValkeyValue;

context.expect_call(
    "CONFIG",
    &["GET", "hz"],
    ValkeyValue::Array(vec![
        ValkeyValue::SimpleString("hz".into()),
        ValkeyValue::SimpleString("10".into()),
    ]),
);
```

For [`Context::config_get()`], use `expect_config_get("hz", "10")` instead of configuring the underlying `CONFIG GET` reply directly.

Use `ThreadSafeContext::test()` to configure a thread-safe context and exercise code through its locked `ContextGuard`:

```rust
use valkey_module::{ThreadSafeContext, ValkeyValue};

let mut context = ThreadSafeContext::test();
context.expect_call(
    "INCR",
    &["counter"],
    ValkeyValue::Integer(1),
);

let reply = context.with_lock(|guard| guard.call("INCR", &["counter"]));
assert!(matches!(reply, Ok(ValkeyValue::Integer(1))));
```

The fixture synchronizes its test state so it can be moved between threads. Each lock receives an independent guard context with a fresh copy of the configured expectations, so expectations can be reused across repeated or concurrent locks. Concurrent test guards may coexist because the shim does not model Valkey's process-wide GIL or command scheduling; this behavior tests fixture isolation, not production locking guarantees. The shim supports only `ThreadSafeContext<DetachedFromClient>` and does not simulate a `ThreadSafeContext` associated with a `BlockedClient`.

Calls to `set_module_options` are accepted as a no-op because their effects require a running server. `Context::create_string()` also works with a test context:

```rust
let context = Context::test();
let value = context.create_string("hello");

assert_eq!(value.as_slice(), b"hello");
```

Use `CommandFilterCtx::test()` to create a `TestCommandFilterCtx` for testing command-filter logic. The test wrapper dereferences to `CommandFilterCtx`, so it can be passed directly to a helper that contains the filter behavior:

```rust
use valkey_module::CommandFilterCtx;

fn rewrite_set(context: &CommandFilterCtx) {
    if context.args_count() == 3
        && context.cmd_get_try_as_str() == Ok("SET")
    {
        context.arg_replace(1, "new-key");
    }
}

let mut context = CommandFilterCtx::test();
context
    .expect_args(&["SET", "key", "value"])
    .expect_get_client_id(42);

rewrite_set(&context);

assert_eq!(
    context.args(),
    vec![b"SET".as_slice(), b"new-key", b"value"]
);
assert_eq!(context.get_client_id(), 42);
```

To test a raw command-filter callback directly, pass `context.as_raw_ctx_ptr()` to it. The returned opaque pointer remains valid while the `TestCommandFilterCtx` is alive:

```rust
use valkey_module::{CommandFilterCtx, RedisModuleCommandFilterCtx};

fn rewrite_set_filter(ctx: *mut RedisModuleCommandFilterCtx) {
    let context = CommandFilterCtx::new(ctx);
    context.arg_replace(1, "new-key");
}

let mut context = CommandFilterCtx::test();
context.expect_args(&["SET", "key", "value"]);

rewrite_set_filter(context.as_raw_ctx_ptr());

assert_eq!(
    context.args(),
    vec![b"SET".as_slice(), b"new-key", b"value"]
);
```

`expect_args()` configures the complete binary-safe argument list and its reported count, while `args()` returns the current list in position order for assertions. Use `expect_args_count()` and `expect_arg_get()` when a test needs to configure those values independently. The test context also supports command lookup, client ID lookup, and argument replacement, insertion, and deletion. Insertions and deletions update the argument count and shift subsequent arguments.

Run these tests normally:

```sh
cargo test
```

The first call to `Context::test()`, `ThreadSafeContext::test()`, `CommandFilterCtx::test()`, `InfoContext::test()`, or `ValkeyString::test()` installs process-wide test callbacks. Create ordinary test contexts and invoke their callbacks on the same thread: live contexts are tracked in thread-local registries, so callbacks on another thread reject the context pointer as foreign. `ThreadSafeContext::test()` is the exception: its synchronized fixture can move to another thread before it is locked. Do not invoke the test shims inside a running Valkey process; setup rejects installation after the real Valkey API has been initialized. Only explicitly shimmed APIs work without Valkey. Other APIs, including `RedisModule_GetServerInfo` and `ValkeyString::append()`, still require a running server.

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
