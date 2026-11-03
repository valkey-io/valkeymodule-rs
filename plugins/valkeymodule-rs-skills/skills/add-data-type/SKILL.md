---
name: add-data-type
description: Add or evolve a native key value type in a downstream valkeymodule-rs module, including descriptor, ownership, persistence contract, and server lifecycle tests. Use for ValkeyType-backed values, not plain domain structs.
---

# Add or evolve a native data type

Read [shared architecture and testing](../../references/module-architecture-and-testing.md).
Ground decisions in ValkeyType (SDK source: `src/native_types.rs`), key wrappers (SDK source: `src/key.rs`), and the existing native type example (SDK source: `examples/data_type.rs`).
The examples omit persistence callbacks and are not complete durable-storage designs.

## Wizard

Follow the shared [wizard protocol](../../references/module-architecture-and-testing.md#wizard-protocol) using this native-type contract table.
Do not invent a type name, value model, encoding, callbacks, or lifecycle behavior.
| Item | Resolve |
| --- | --- |
| Name | What is the stable Valkey type name? It must be explicit and valid for the resolved SDK and server. |
| Value model | What Rust value does the type own, which commands may create or mutate it, and what are its missing-key and wrong-type behaviors? |
| Persistence | Is the type durable? If so, specify RDB/AOF format, encoding version, malformed/old-data handling, and compatibility or migration requirements. |
| Lifecycle | Which copy, free, digest, memory-usage, defrag, expiry, and delete behaviors apply? Explicitly name unsupported capabilities. |
| Replication and target | What replication behavior, memory-accounting requirements, and minimum server/SDK target apply? |

Use the table as the internal checklist. The Name item is always applicable; apply the shared [explicit-name requirement](../../references/module-architecture-and-testing.md#require-an-explicit-name-before-implementation). After confirmation, record the chosen type and encoding contract before changing representation.

## Code samples

Use the native-type example (SDK source: `examples/data_type.rs`) only as a registration and key-access reference. Adapt descriptor methods, raw ownership, persistence callbacks, and server tests to the confirmed contract; its omitted callbacks are not a durable-storage design.

## Implementation

1. Resolve the [type contract](#wizard) before implementation.
2. Create a type-specific file or subdirectory under `types/`, with small `mod.rs` declarations/re-exports.
   Separate substantial encoding and lifecycle callbacks from domain operations.
   Keep command parsing/reply mapping in `commands/`, and import the descriptor identifier into `lib.rs` for its `data_types` entry.
3. Define the `ValkeyType` descriptor and the actual `RedisModuleTypeMethods` supported by the resolved SDK/engine.
   The local SDK checks a nine-byte name length before creating the type; choose an appropriate stable ASCII type name and verify server acceptance.
   Distinguish the type's encoding version from the methods-structure version.
   Do not casually rename a persisted type or reuse an encoding version for an incompatible format.
4. Trace every ownership transfer.
   Writable `set_value` boxes the Rust value and hands a raw pointer to the module API; its type must match the descriptor and free/persistence callbacks.
   `get_value` returns `Option` after type checking; do not keep returned references beyond the key/callback lifetime or create conflicting mutable aliases just because wrapper lifetimes permit it.
   Inspect success/failure cleanup in the actual SDK; do not assume an error automatically reclaims a handed-off allocation.
5. Design only the needed methods, explicitly recording unsupported capabilities.
   If durability is required, supply and test RDB load/save and the applicable AOF/replication behavior.
   Plan malformed/old encoding handling and partial-load cleanup.
   For free, copy, digest, memory usage or defrag callbacks, use the right raw value type, prevent double frees, bound work and prevent panics crossing the C ABI.
   Do not blindly copy an example's `None` callbacks into a durable type.
6. Follow the shared [testing guidance](../../references/module-architecture-and-testing.md#testing) and unit-test domain invariants and serialization decisions that can run on ordinary data.
   Current test shims do not simulate module key storage, native-type registration or persistence.
   Use isolated server tests for empty/wrong-type keys, create/update/read/delete, lifecycle cleanup, and the required persistence/reload/copy/expiry paths.
   For a migration, include old stored data as a fixture in the downstream project's existing test strategy.
7. Apply the shared structural completion check plus an ownership review of every raw allocation path, then follow [implementation verification](../../references/module-architecture-and-testing.md#implementation-verification).
   Report the type/encoding contract, registrations, tested lifecycle paths and any deferred persistence support.
   Do not claim durability or leak freedom from unit tests of domain data alone.
