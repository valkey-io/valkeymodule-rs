---
name: add-config
description: Add or change a registered configuration setting in a downstream valkeymodule-rs module, including storage, validation, update callbacks, and rejection tests. Use for module configuration rather than ordinary command arguments.
---

# Add or change module configuration

Read [shared architecture and testing](../../references/module-architecture-and-testing.md), especially its configuration mutation-order warning.
Inspect configuration implementation (SDK source: `src/configuration.rs`), registration expansion (SDK source: `src/macros.rs`), and configuration example (SDK source: `examples/configuration.rs`) before choosing callbacks.

## Wizard

Follow the shared [wizard protocol](../../references/module-architecture-and-testing.md#wizard-protocol) using this configuration-contract table.
Do not invent defaults, bounds, variants, flags, or side effects.

| Item | Resolve |
| --- | --- |
| Name | What is the public configuration name? It must be explicit; do not derive or invent one. |
| Type | Choose exactly one: `i64`, `string`, `bool`, or `enum`. |
| `i64` value, if selected | What signed default and inclusive minimum and maximum apply, and should users be able to supply memory units? |
| `i64` storage, if unresolved | Does the module need `AtomicI64` access or `ValkeyGILGuard<i64>` access for this value? |
| `string` value, if selected | What default, length or format restrictions, and allowed values apply, and must the module preserve bytes that are not valid UTF-8? |
| `string` storage, if unresolved | Should the module use `Mutex<String>`, `ValkeyGILGuard<String>`, or `ValkeyGILGuard<ValkeyString>` for its required access pattern? |
| `bool` value, if selected | Is the default `true` or `false`, and is either value or transition disallowed? |
| `bool` storage, if unresolved | Does the module need `AtomicBool` access or `ValkeyGILGuard<bool>` access? |
| `enum` value, if selected | What are every option's name and distinct numeric value, which option is the default, and may options be combined as bit flags? |
| `enum` storage, if unresolved | Should the module use `Mutex<Enum>` or `ValkeyGILGuard<Enum>` for its required access pattern? |
| Availability | Is it startup-only (`IMMUTABLE`) or runtime-configurable (`DEFAULT`)? |
| Runtime mutation, if applicable | May the value change after it is first set, which candidate values or transitions must be rejected, what error should clients see, and must rejection leave stored state unchanged? |
| Accepted-change effects, if applicable | Which accepted changes need an `on_changed` callback, and what observable effect should it produce? |
| Flags | Which applicable modifiers from the `ConfigurationFlags` table below should be combined with the availability flag? |
| Load arguments | Should module-load arguments initialize this setting? This determines `module_args_as_configuration` and applies to the registration set, not an individual tuple. |

Use the table as the internal checklist.
If args are passed to module using `module_args` pass them to TestServer::start but don't create a separate `start_with_server_args` function.
Use `on_changed` and `on_set` callbacks to implement validation logic for configs.

### ConfigurationFlags

The resolved SDK's `ConfigurationFlags` definition (SDK source: `src/configuration.rs`) is authoritative if it differs from this reference.

| Flag | Meaning | Applicable setting types |
| --- | --- | --- |
| `DEFAULT` | Uses no restriction bits and permits changes after startup. | All |
| `IMMUTABLE` | Allows the value to be set only when the module loads. | All |
| `SENSITIVE` | Redacts the value from logging. | All |
| `HIDDEN` | Hides the name from pattern-matching `CONFIG GET` requests. | All |
| `PROTECTED` | Makes changes subject to the server's `enable-protected-configs` policy. | All |
| `DENY_LOADING` | Forbids changes while the server is loading data. | All |
| `MEMORY` | Accepts memory-unit notation and converts it to bytes. | Numeric (`i64`) |
| `BITFLAGS` | Allows multiple enum options to be combined as bit flags. | Enum |

Choose `DEFAULT` for runtime updates or `IMMUTABLE` for a startup-only setting, then combine applicable modifiers with `|`.
`DEFAULT` has zero bits, so it is optional in a combination, but including it makes the runtime choice visible.
For example, a runtime `i64` setting that accepts memory units, is hidden from pattern-matching `CONFIG GET`, and cannot change while data loads uses:

```rust
ConfigurationFlags::DEFAULT
    | ConfigurationFlags::MEMORY
    | ConfigurationFlags::HIDDEN
    | ConfigurationFlags::DENY_LOADING
```

Use `MEMORY` only for numeric (`i64`) settings and `BITFLAGS` only for enum settings.

## Code samples

This example registers four independent settings for a module named `my-module`.
Populate only the selected config types, but include all four type sections in `configurations`.
Use empty lists for unused types; the SDK macro needs these placeholders when registering a subset of config types.
Replace the illustrative names and values with input provided by user.
All four use the SDK's built-in storage implementations; this sample adds no business logic or callbacks.

### `src/config/mod.rs`

```rust
use std::sync::{
    atomic::{AtomicBool, AtomicI64},
    Mutex,
};

use valkey_module::enum_configuration;

enum_configuration! {
    pub(crate) enum MyEnum {
        ValueOne = 0,
        ValueTwo = 1,
    }
}

pub(crate) static MY_INT: AtomicI64 = AtomicI64::new(10);
pub(crate) static MY_STRING: Mutex<String> = Mutex::new(String::new());
pub(crate) static MY_BOOLEAN: AtomicBool = AtomicBool::new(true);
pub(crate) static MY_ENUM: Mutex<MyEnum> = Mutex::new(MyEnum::ValueOne);
```

The SDK checks the registered integer minimum and maximum.
The string storage is a `Mutex<String>`, so it does not create a server-backed `ValkeyString` before the module API is ready.
There is no pure logic to unit-test in this registration-only example; the server integration test verifies the settings.

### Register in `src/lib.rs`

Declare `mod config;`, import `config::{MyEnum, MY_BOOLEAN, MY_ENUM, MY_INT, MY_STRING}` and `valkey_module::configuration::ConfigurationFlags`, and add this field to the existing `valkey_module!` invocation.
Keep the other fields and their required order from the existing module.

```rust
configurations: [
    i64: [
        ["my_int", &MY_INT, 10, 0, 100, ConfigurationFlags::DEFAULT, None],
    ],
    string: [
        ["my_string", &MY_STRING, "default", ConfigurationFlags::DEFAULT, None],
    ],
    bool: [
        ["my_boolean", &MY_BOOLEAN, true, ConfigurationFlags::DEFAULT, None],
    ],
    enum: [
        ["my_enum", &MY_ENUM, MyEnum::ValueOne, ConfigurationFlags::DEFAULT, None],
    ],
    module_args_as_configuration: true,
]
```

These tuple shapes are specific to each type.
`ConfigurationFlags::DEFAULT` permits runtime updates; choose other flags only when the requested contract needs them.
`module_args_as_configuration: true` is appropriate only if module load arguments should initialize these settings.

### Add to `tests/integration.rs`

Keep the existing module-load test and `mod utils;` declaration from `create-module`.
This sample uses its `Result` and `TestServer` imports.
Replace `my-module` with the downstream module name and test only the settings actually added.
Place the shared `config_get` helper at the bottom of `tests/integration.rs`, after the integration tests.

```rust
#[test]
fn configurations_load_and_update() -> Result<()> {
    let server = TestServer::start("my-module")?;
    let mut connection = server.connection()?;

    for (name, default, updated) in [
        ("my-module.my_int", "10", "12"),
        ("my-module.my_string", "default", "ready"),
        ("my-module.my_boolean", "yes", "no"),
        ("my-module.my_enum", "ValueOne", "ValueTwo"),
    ] {
        assert_eq!(config_get(&mut connection, name)?, default);
        let reply: String = redis::cmd("CONFIG")
            .arg("SET")
            .arg(name)
            .arg(updated)
            .query(&mut connection)?;
        assert_eq!(reply, "OK");
        assert_eq!(config_get(&mut connection, name)?, updated);
    }

    Ok(())
}

fn config_get(connection: &mut redis::Connection, name: &str) -> Result<String> {
    let values: Vec<String> = redis::cmd("CONFIG")
        .arg("GET")
        .arg(name)
        .query(connection)?;
    anyhow::ensure!(values.len() == 2, "expected one value for {name}");
    Ok(values[1].clone())
}
```

Use additional server tests for load-time arguments, immutable settings, callbacks, and live side effects when those behaviors are part of the selected contract.
Do not add unit tests merely to retest SDK-provided atomic, mutex, or enum registration behavior; the real-server test proves registration and updates.

## Implementation

1. Resolve the [configuration contract](#wizard), then take only the selected type or types from the [code samples](#code-samples).
   Adapt names, defaults, applicable bounds/options, flags, and load-time arguments; do not copy the illustrative values as requirements.
2. For a small cohesive configuration area, keep storage in `config/mod.rs`, with `lib.rs` for registration.
   Split `config/` into focused setting files only when it grows enough to benefit from them.
   Use the SDK's built-in storage implementation for a simple setting; registration needs a static reference.
   Add validation, callbacks, or domain rules only when the requested behavior needs them.
3. Check the selected type's tuple order, storage generic type, flags, and initialization behavior.
   For server config-change notifications, use [add-event-handler](../add-event-handler/SKILL.md); do not invent a config-rewrite callback.
4. Follow the shared [testing guidance](../../references/module-architecture-and-testing.md#testing): unit-test only custom validation or domain effects when present, and use real-server tests for registration, defaults, accepted updates, and any rejection or live effects required.
   Finish with the shared structural check and [implementation verification](../../references/module-architecture-and-testing.md#implementation-verification); report what passed and what remains unverified.
