---
name: add-filter
description: Add or modify command interception in a downstream valkeymodule-rs module using CommandFilterCtx, with binary-safe rewrite tests and registration lifecycle checks. Use for filters that inspect or rewrite commands before execution.
---

# Add or change a command filter

Read [shared architecture and testing](../../references/module-architecture-and-testing.md).
Inspect filter APIs (SDK source: `src/context/filter.rs`), filter registration macro (SDK source: `src/macros.rs`), existing filter example (SDK source: `examples/filter1.rs`), and filter fixture (SDK source: `src/test-shims/command_filter_ctx.rs`).

## Wizard

Follow the shared [wizard protocol](../../references/module-architecture-and-testing.md#wizard-protocol) using this filter contract table.
Ask questions one at a time
Do not invent anything

| Item | Resolve |
| --- | --- |
| Identifier | What is the explicit Rust filter callback/handler identifier? It is not a public Valkey command name. |
| Scope | Which commands, clients, and binary argument forms are intercepted? Which inputs must pass through unchanged? |
| Rewrite | What exact insertion, deletion, replacement, or pass-through behavior applies, including malformed input and arity cases? |
| Compatibility and tests | What server/SDK feature target applies, and what fixture and real-server cases prove argument handling, interception, and unload? |

Use the table as the internal checklist. The Identifier item is always applicable; apply the shared [explicit-name requirement](../../references/module-architecture-and-testing.md#require-an-explicit-name-before-implementation).

## Code samples

Use the filter example (SDK source: `examples/filter1.rs`) and filter fixture (SDK source: `src/test-shims/command_filter_ctx.rs`) as API references.
These templates leave commands unchanged until the requested filter behavior is implemented.
Replace `my_filter` with the confirmed callback identifier and adapt the tests when generating the actual code.

### `src/filters/my_filter.rs`

```rust
use valkey_module::{CommandFilterCtx, RedisModuleCommandFilterCtx};

pub(crate) fn my_filter(ctx: *mut RedisModuleCommandFilterCtx) {
    let _context = CommandFilterCtx::new(ctx);
    // Add filter logic here.
}
```

### `src/filters/mod.rs`

```rust
mod my_filter;

pub(crate) use my_filter::my_filter;
```

### Register in `src/lib.rs`

Declare the filters module and import the callback and registration flag.

```rust
mod filters;

use filters::my_filter;
use valkey_module::VALKEYMODULE_CMDFILTER_NOSELF;
```

Add the callback to the existing `valkey_module!` filter list, after `commands` and before any `event_handlers` or `configurations` fields.
Keep existing filter entries and register each callback once.

```rust
filters: [
    [my_filter, VALKEYMODULE_CMDFILTER_NOSELF],
],
```

The SDK macro uses `paste` to generate the callback adapter, so add this normal dependency if it is missing from `Cargo.toml`.

```toml
[dependencies]
paste = "1"
```

### Unit test sample

Add this template at the bottom of `src/filters/my_filter.rs`.
Enable the SDK's `test-shims` development feature as described in the [shared testing guidance](../../references/module-architecture-and-testing.md#testing).
The test calls the placeholder filter and checks that command arguments, including binary bytes, remain unchanged.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_leaves_arguments_unchanged() {
        let mut context = CommandFilterCtx::test();
        let args = [b"ECHO".as_slice(), b"\0\xff".as_slice()];
        context.expect_args(&args);

        my_filter(context.as_raw_ctx_ptr());

        assert_eq!(context.args(), args.to_vec());
    }
}
```

### Integration test sample

Add this template to the existing `tests/integration.rs`, keeping its `mod utils;` declaration and `Result` and `TestServer` imports from `create-module`.
Replace `my-module` with the downstream module name.
This high-level test checks that a command still works with the module loaded; add assertions for actual interception behavior when implementing the filter.

```rust
#[test]
fn command_succeeds_with_filter_module_loaded() -> Result<()> {
    let server = TestServer::start("my-module", &[])?;
    let mut connection = server.connection()?;
    let input = b"\0\xff";

    let reply: Vec<u8> = redis::cmd("ECHO")
        .arg(input.as_slice())
        .query(&mut connection)?;

    assert_eq!(reply, input.to_vec());
    Ok(())
}
```

## Implementation

1. Resolve the [filter contract](#wizard) before implementation.
   Add `paste` crate
2. Put each filter or cohesive rewrite in a file under `filters/`; use `filters/mod.rs` for declarations/re-exports and narrowly scoped lifecycle coordination.
   Keep reusable rewrite decisions with the owning filter feature.
   Replacement command belong in `commands/`; `lib.rs` contains registration only.
3. Register filters via `valkey_module!` macro.
   Specify `VALKEYMODULE_CMDFILTER_NOSELF` when registering filter.  
   Do not use `Context::register_command_filter` and `unregister_command_filter`.
4. Follow the shared [testing guidance](../../references/module-architecture-and-testing.md#testing) and unit-test through `CommandFilterCtx::test`: configure complete bytes with `expect_args`, call the adapter with `as_raw_ctx_ptr` while the fixture lives on the same thread, then assert `args`.
5. Run server tests for registration, actual interception order/flags, replacement-command behavior, recursion avoidance and unload.
   Shims test argument manipulation, not server interception or handle lifetime.
   Check ownership/error paths and the shared structural completion criteria, then follow [implementation verification](../../references/module-architecture-and-testing.md#implementation-verification); report the test boundary honestly.
