---
name: add-module-lifecycle
description: Use when adding or changing init, deinit or preload hooks in a downstream valkeymodule-rs module.
---

# Add init and deinit

Read [shared architecture and testing](../../references/module-architecture-and-testing.md), then inspect the module's existing registration style.
Inspect the SDK's lifecycle examples (SDK source: `examples/preload.rs` and `examples/load_unload.rs`) and entry macro (SDK source: `src/macros.rs`).

## Wizard

Follow the shared [wizard protocol](../../references/module-architecture-and-testing.md#wizard-protocol) using this contract table.

| Item | Resolve |
| --- | --- |
| Init | Should there be init function? Fires after module is loaded.  |
| Deinit | Should there be deinit function?  Fires before module is unloaded.  |
| Preload | Should there be preload function? Fires right after module is loaded but before anything else.  |

## Code samples

Put `init`, `deinit`, and `preload` in `src/lifecycle/mod.rs` and register the selected hooks in `src/lib.rs`.

### `src/lifecycle/mod.rs`

```rust
use valkey_module::{Context, Status, ValkeyString};

pub(crate) fn init(ctx: &Context, args: &[ValkeyString]) -> Status {
    // add logic here
    Status::Ok
}

pub(crate) fn deinit(ctx: &Context) -> Status {
    // add logic here
    Status::Ok
}

pub(crate) fn preload(ctx: &Context, args: &[ValkeyString]) -> Status {
    // add logic here
    Status::Ok
}
```

### Register in `src/lib.rs`

Declare the lifecycle module and import only the hooks the module uses.

```rust
mod lifecycle;

use lifecycle::{deinit, init, preload};
```

Add the selected hook fields to the existing `valkey_module!` invocation after `data_types` and before any `info`, `auth`, `acl_categories`, or `commands` fields.

```rust
preload: preload,
init: init,
deinit: deinit,
```

### Unit test sample

Add this template at the bottom of `src/lifecycle/mod.rs`, keeping only the tests for hooks the module uses.
Enable the SDK's `test-shims` development feature as described in the [shared testing guidance](../../references/module-architecture-and-testing.md#testing).
These high-level tests call the placeholder hooks with a test context and assert successful completion.
Adapt their arguments and assertions when generating the actual lifecycle implementation.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_succeeds() {
        let context = Context::test();

        assert_eq!(init(&context, &[]), Status::Ok);
    }

    #[test]
    fn deinit_succeeds() {
        let context = Context::test();

        assert_eq!(deinit(&context), Status::Ok);
    }

    #[test]
    fn preload_succeeds() {
        let context = Context::test();

        assert_eq!(preload(&context, &[]), Status::Ok);
    }
}
```

### Integration test sample

Add this template to the existing `tests/integration.rs`, keeping its `mod utils;` declaration and `Result` and `TestServer` imports from `create-module`.
Replace `my-module` with the downstream module name and pass any required module-load arguments to `TestServer::start`.
Ensure the isolated server fixture permits `MODULE UNLOAD` on the target server.
This high-level test checks successful loading and unloading; adapt its assertions when generating the actual lifecycle implementation.

```rust
#[test]
fn module_loads_and_unloads() -> Result<()> {
    let module_name = "my-module";
    let server = TestServer::start(module_name, &[])?;
    let mut connection = server.connection()?;

    let modules: Vec<redis::Value> = redis::cmd("MODULE")
        .arg("LIST")
        .query(&mut connection)?;
    assert!(format!("{modules:?}").contains(module_name));

    let reply: String = redis::cmd("MODULE")
        .arg("UNLOAD")
        .arg(module_name)
        .query(&mut connection)?;
    assert_eq!(reply, "OK");

    let modules: Vec<redis::Value> = redis::cmd("MODULE")
        .arg("LIST")
        .query(&mut connection)?;
    assert!(!format!("{modules:?}").contains(module_name));
    Ok(())
}
```

## Implementation

Resolve the [lifecycle contract](#wizard) before using the [code samples](#code-samples).
Keep small hooks and their unit tests together in `src/lifecycle/mod.rs`; split substantial hook implementations into focused files under `src/lifecycle/` when needed.
Keep `src/lib.rs` focused on declarations, imports, and registration, and keep integration tests in `tests/integration.rs`.
Finish with the shared [implementation verification](../../references/module-architecture-and-testing.md#implementation-verification) and report which lifecycle paths were exercised.
