---
name: add-event-handler
description: Add or change server-event callbacks in a downstream valkeymodule-rs module, with subscription, payload, lifecycle and testing checks. Route deferred work to workers only when the event behavior requires it.
---

# Add or change a server-event handler

Read [shared architecture and testing](../../references/module-architecture-and-testing.md).
Inspect server-event registration (SDK source: `src/macros.rs`), server-event implementation (SDK source: `src/context/server_events.rs`), event attributes (SDK source: `valkeymodule-rs-macros/src/lib.rs`), and the server-event example (SDK source: `examples/server_events.rs`).

## Wizard

Follow the shared [wizard protocol](../../references/module-architecture-and-testing.md#wizard-protocol) using this event-handler contract table.
If the user has not specified the event, first ask: "Which event should this handler handle?"
Offer relevant choices from [Choose the event path](#choose-the-event-path), then wait for the user's selection before asking the remaining questions or generating code.
If the event is already explicit, use that selection without asking again.
After selecting the event, ask which subevents apply if that scope is still unresolved.
Do not invent an event path, payload, state effect, registration metadata, or server target.

| Item | Resolve |
| --- | --- |
| Event | Which server event is this handler for? |
| Trigger scope | Which subevents should it handle, such as client connected/disconnected or flush started/ended? |
| Payload and target | What payload is required, and which Valkey/Redis targets must support the selected callback? |
| Effect | What state change, client-visible effect, failure policy, and reentrancy behavior are required? |
| Execution | Must work run in the callback or through a worker? State retained-data ownership and shutdown behavior. |
| Registration and tests | Which server-event attribute applies, and what unit and server coverage proves delivery and effects? |

Use the table as the internal checklist.
Generate only the selected event handler and its applicable registration and test templates.
Confirm the selected SDK mechanism against its actual callback signature before implementation.

## Code samples

Use `examples/server_events.rs` for server-event attributes and payloads.
The templates below contain no business logic; generate only the selected handlers, imports, registrations, and matching tests.
Rename the illustrative callbacks to the confirmed identifiers and adapt the tests when implementing actual behavior.

### Choose the event path

| Need | Local SDK mechanism and payload |
| --- | --- |
| Role, loading, flush, module or client changes | Attribute handlers `role_changed_event_handler`, `loading_event_handler`, `flush_event_handler`, `module_changed_event_handler`, `client_changed_event_handler`; typed payloads are `ServerRole`, `LoadingSubevent`, `FlushSubevent`, `ModuleChangeSubevent`, `ClientChangeSubevent` from `valkey_module::server_events`. |
| Configuration changes | `config_changed_event_handler` receives context and changed names as `&[&str]`; this is separate from a setting's validation/update callbacks. There is no dedicated config-rewrite handler in the local high-level API; do not invent one. |
| Server key lifecycle | `key_event_handler` receives `KeyChangeSubevent`; this wrapper does not supply the key name. |
| Persistence, master link, fork child, replica or asynchronous replication loading | `persistence_event_handler`, `master_link_change_event_handler`, `fork_child_event_handler`, `replica_change_event_handler`, `repl_async_load_event_handler`; verify their corresponding subevent enums in the implementation. |
| Loading progress or event-loop phases | `loading_progress_event_handler` receives `(&Context, LoadingProgress)`; `event_loop_event_handler` receives context and `EventLoopSubevent`. The loading-progress macro's comment describing separate parameters is stale. |
| Cron, shutdown or swap DB | `cron_event_handler`, `shutdown_event_handler`, `swapdb_event_handler` use context plus a `u64`. Inspect the actual callback before depending on the integer's meaning; the current adapters do not expose every raw event data field. |

This is an inventory of wrappers in the local SDK, not a promise that every event exists on every supported server version.

### `src/handlers/server_events.rs`

This is a catalogue of the SDK's server-event callback signatures.
Keep only the handlers needed for the selected contract and supported by the target server.
Split selected handlers into focused files under `src/handlers/` when their implementations grow.

```rust
use valkey_module::server_events::{
    ClientChangeSubevent, EventLoopSubevent, FlushSubevent, ForkChildSubevent,
    KeyChangeSubevent, LoadingProgress, LoadingSubevent, MasterLinkChangeSubevent,
    ModuleChangeSubevent, PersistenceSubevent, ReplAsyncLoadSubevent,
    ReplicaChangeSubevent, ServerRole,
};
use valkey_module::Context;
use valkey_module_macros::{
    client_changed_event_handler, config_changed_event_handler, cron_event_handler,
    event_loop_event_handler, flush_event_handler, fork_child_event_handler,
    key_event_handler, loading_event_handler, loading_progress_event_handler,
    master_link_change_event_handler, module_changed_event_handler,
    persistence_event_handler, repl_async_load_event_handler,
    replica_change_event_handler, role_changed_event_handler,
    shutdown_event_handler, swapdb_event_handler,
};

#[role_changed_event_handler]
fn on_role_changed(_ctx: &Context, _role: ServerRole) {
    // Add role-change logic here.
}

#[loading_event_handler]
fn on_loading(_ctx: &Context, _event: LoadingSubevent) {
    // Add loading-event logic here.
}

#[flush_event_handler]
fn on_flush(_ctx: &Context, _event: FlushSubevent) {
    // Add database-flush logic here.
}

#[module_changed_event_handler]
fn on_module_changed(_ctx: &Context, _event: ModuleChangeSubevent) {
    // Add module-change logic here.
}

#[client_changed_event_handler]
fn on_client_changed(_ctx: &Context, _event: ClientChangeSubevent) {
    // Add client-change logic here.
}

#[config_changed_event_handler]
fn on_config_changed(_ctx: &Context, _changed_configs: &[&str]) {
    // Add configuration-change logic here.
}

#[key_event_handler]
fn on_key_changed(_ctx: &Context, _event: KeyChangeSubevent) {
    // Add key-lifecycle logic here.
}

#[persistence_event_handler]
fn on_persistence(_ctx: &Context, _event: PersistenceSubevent) {
    // Add persistence-event logic here.
}

#[master_link_change_event_handler]
fn on_master_link_changed(_ctx: &Context, _event: MasterLinkChangeSubevent) {
    // Add replication-link logic here.
}

#[fork_child_event_handler]
fn on_fork_child(_ctx: &Context, _event: ForkChildSubevent) {
    // Add fork-child logic here.
}

#[replica_change_event_handler]
fn on_replica_changed(_ctx: &Context, _event: ReplicaChangeSubevent) {
    // Add replica-change logic here.
}

#[repl_async_load_event_handler]
fn on_repl_async_load(_ctx: &Context, _event: ReplAsyncLoadSubevent) {
    // Add asynchronous replication-load logic here.
}

#[loading_progress_event_handler]
fn on_loading_progress(_ctx: &Context, _progress: LoadingProgress) {
    // Add loading-progress logic here.
}

#[event_loop_event_handler]
fn on_event_loop(_ctx: &Context, _event: EventLoopSubevent) {
    // Add event-loop logic here.
}

#[cron_event_handler]
fn on_cron(_ctx: &Context, _hz: u64) {
    // Add cron-event logic here.
}

#[shutdown_event_handler]
fn on_shutdown(_ctx: &Context, _subevent: u64) {
    // Add server-shutdown logic here.
}

#[swapdb_event_handler]
fn on_swapdb(_ctx: &Context, _subevent: u64) {
    // Add database-swap logic here.
}
```

### `src/handlers/mod.rs`

Declare only the files generated for the selected event handlers.
Attribute-based server handlers are discovered when their module is compiled and do not need re-exports for registration.

```rust
mod server_events;
```

### Register in `src/lib.rs`

Declare `mod handlers;` so the handler files are compiled.
For attribute-based server handlers, the existing `valkey_module!` invocation handles registration without an `event_handlers` entry or an additional call from `init`.
Add these normal dependencies when using the server-event attributes, matching the macro crate's version and source to the resolved SDK.
The version below matches the current SDK examples.

```toml
[dependencies]
valkey-module-macros = "0.1.14"
linkme = "0.3"
```

### Unit test samples

Enable the SDK's `test-shims` development feature using the [shared testing guidance](../../references/module-architecture-and-testing.md#testing).
These callbacks return `()`, so the placeholder tests simply call them and succeed if they return without panicking.
They do not simulate server subscriptions or event delivery.
Keep only the calls and imports corresponding to selected handlers.

Add this template at the bottom of `src/handlers/server_events.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use valkey_module::server_events::LoadingProgressSubevent;

    #[test]
    fn typed_handlers_accept_sample_events() {
        let context = Context::test();

        on_role_changed(&context, ServerRole::Primary);
        on_loading(&context, LoadingSubevent::RdbStarted);
        on_flush(&context, FlushSubevent::Started);
        on_module_changed(&context, ModuleChangeSubevent::Loaded);
        on_client_changed(&context, ClientChangeSubevent::Connected);
        on_key_changed(&context, KeyChangeSubevent::Deleted);
        on_persistence(&context, PersistenceSubevent::RdbStart);
        on_master_link_changed(&context, MasterLinkChangeSubevent::Up);
        on_fork_child(&context, ForkChildSubevent::Born);
        on_replica_changed(&context, ReplicaChangeSubevent::Online);
        on_repl_async_load(&context, ReplAsyncLoadSubevent::Started);
        on_event_loop(&context, EventLoopSubevent::BeforeSleep);
    }

    #[test]
    fn handlers_accept_sample_payloads() {
        let context = Context::test();

        on_config_changed(&context, &["maxmemory"]);
        on_loading_progress(
            &context,
            LoadingProgress {
                subevent: LoadingProgressSubevent::Rdb,
                hz: 10,
                progress: 0,
            },
        );
        on_cron(&context, 10);
        on_shutdown(&context, 0);
        on_swapdb(&context, 0);
    }
}
```

### Integration test samples

Add the applicable templates to `tests/integration.rs`, keeping its `mod utils;` declaration and `Result` and `TestServer` imports from `create-module`.
Replace `my-module` with the downstream module name and load only handlers supported by the target server.
These tests perform basic server operations with the placeholder handlers loaded and assert normal command results.
They do not prove that a particular callback ran; add delivery and effect assertions when generating actual handler behavior.

```rust
#[test]
fn event_module_accepts_client_connections() -> Result<()> {
    let server = TestServer::start("my-module", &[])?;
    let mut connection = server.connection()?;

    let reply: String = redis::cmd("PING").query(&mut connection)?;

    assert_eq!(reply, "PONG");
    Ok(())
}

#[test]
fn key_lifecycle_operations_succeed_with_handler_loaded() -> Result<()> {
    let server = TestServer::start("my-module", &[])?;
    let mut connection = server.connection()?;

    let reply: String = redis::cmd("SET")
        .arg("sample-key")
        .arg("sample-value")
        .query(&mut connection)?;
    assert_eq!(reply, "OK");

    let removed: i64 = redis::cmd("DEL")
        .arg("sample-key")
        .query(&mut connection)?;
    assert_eq!(removed, 1);
    Ok(())
}

#[test]
fn database_flush_succeeds_with_handler_loaded() -> Result<()> {
    let server = TestServer::start("my-module", &[])?;
    let mut connection = server.connection()?;

    let reply: String = redis::cmd("FLUSHDB").query(&mut connection)?;

    assert_eq!(reply, "OK");
    Ok(())
}
```

## Implementation

1. Resolve the [event-handler contract](#wizard) before implementation.
   Choose the appropriate [server-event path](#choose-the-event-path).
2. Put each event adapter or cohesive callback feature in a file under `handlers/`; keep `handlers/mod.rs` to declarations/re-exports/dispatch wiring.
   Client-event, config-change and flush-DB adapters belong here.
   Keep setting definitions/validation/update behavior under `config/` and delegate from adapters; keep core state transitions with their owning capability.
   Keep worker ownership with its owning feature.
   Reusable miscellaneous helpers may use focused `utils/` files, never a second home for capability logic.
   Leave top-level registration in `lib.rs`.
3. Apply the matching server-event attribute and ensure the handler's module is compiled and its direct `linkme`/macro dependencies are present.
   The entry macro calls `register_server_events`; it subscribes nonempty handler lists and propagates subscription errors.
   Do not call the registrar again from `init` or assume startup silently ignores an unsupported server event.
   Verify the engine minimum for the chosen callback.
4. Keep callbacks bounded and borrowed payloads local.
   Avoid panics and recursive notification loops.
   Deferred work must own any retained data.
   Do not assume every server-event context permits every API operation.
5. If workers are needed, follow the shared conditional lifecycle guidance: own handles and cancellation, keep real server operations under the appropriate guard, and make unload terminate work without deadlocks.
   A shutdown event is not a substitute for module `deinit` cleanup; module unload and server shutdown are different triggers.
6. Test pure state transitions and directly callable callback logic with supported fixtures where applicable.
   No current shim simulates event subscriptions or server event delivery.
   Follow the shared [testing guidance](../../references/module-architecture-and-testing.md#testing) and run server tests for registration and trigger delivery, payload effects, unsupported targets, reentrancy prevention and shutdown/unload as applicable.
   Use bounded waits for observable effects.
7. Apply the shared structural completion check and [implementation verification](../../references/module-architecture-and-testing.md#implementation-verification), then report the chosen event mechanism, payload limitations, owned state, lifecycle decisions and actual test coverage.
   Do not claim raw-event payload coverage from a wrapper that discards that data.
