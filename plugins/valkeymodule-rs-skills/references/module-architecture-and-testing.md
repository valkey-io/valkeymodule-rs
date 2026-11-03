# Module architecture and testing with valkeymodule-rs

Start with the valkeymodule-rs SDK source and downstream module guidance below, then read the structure, code samples, and testing sections relevant to each workflow.

## Resolve the downstream module and SDK

1. Identify the downstream module project from the active task.
   That project is the edit target.
   Record any explicit SDK checkout path supplied in the task or project instructions.
2. Inspect the downstream dependencies in `Cargo.toml` and `Cargo.lock`.
   Do not fetch crates or rewrite a lockfile.
3. Validate SDK root by its `Cargo.toml`.
   If the SDK cannot be found or read, request its local or HTTP path or specific version on crates.io

## Require an explicit name before implementation

For `create-module`, `add-command`, `add-data-type`, and `add-filter`, request a name before writing any code.
Convert all user input into snake_case and lowercase it.  
If it is missing, ask before proceeding; do not infer it from examples, filenames, or implementation details.
Each workflow defines what its name identifies: the module/crate, public Valkey command, stable native type name, or filter callback/handler identifier.

## Keep registrations and handler names unique

Before adding or renaming a command, filter, configuration, native data type, or event handler, inspect existing registrations in the downstream module.
If the proposed name already belongs to a different feature, ask for a distinct name before implementation; changing an existing feature may keep its own name.

| Feature | Uniqueness check |
| --- | --- |
| Command | Keep the public command name distinct from other module commands and commands already available on the target server; account for the server's case-insensitive command lookup. |
| Configuration | Keep the public configuration name distinct from other settings in the module and on the target server; compare the fully qualified name exposed through `CONFIG GET` and `CONFIG SET`. |
| Native data type | Keep the native type name distinct from other registered type names, including names used by loaded modules or persisted data; do not rename an existing persisted type merely to resolve a collision. |
| Filter | Give each distinct filter callback a unique Rust identifier in its registration scope, and do not register the same intended filter twice; filters have no separate public Valkey name. |
| Event handler | Give each distinct callback a unique Rust identifier in its registration scope, and avoid duplicate registration of the same handler; multiple handlers may intentionally subscribe to the same event or keyspace trigger. |

Check planned names against each other as well as existing registrations, then verify server-visible names and intended registrations in real-server integration tests.

## Wizard protocol

When a focused skill provides a Wizard table, first inspect it and the active module.
Each focused skill defines its own applicable items and capability-specific constraints.
Ask only one question per user turn: ask the first applicable unanswered item, then wait before asking the next. 
Do not send the table as a checklist or prompt template. 
Phrase each question concisely and combine only details that must be decided together, such as a numeric default with its bounds or a key position with its access.
Once all question are answered, summarize the details and ask for explicit confirmation before implementation. 

## Code placement before implementation

Use Rust module **directories**, each with a small `mod.rs` for declarations, narrow re-exports, and essential wiring.
Put individual commands, filters, handlers, or cohesive features in separate files inside the directory.
Do not replace a large `lib.rs` with equally large `commands.rs`, `filters.rs`, or `mod.rs` files.

| Area in the downstream crate | Responsibility |
| --- | --- |
| `src/lib.rs` | Declare top-level modules; import registration names and lifecycle hooks; invoke `valkey_module!`. |
| `src/lifecycle/mod.rs` and focused lifecycle files when needed | Module `preload`, `init`, and `deinit` hooks, lifecycle coordination, and colocated unit tests. |
| `src/commands/mod.rs` and command/feature files | Argument parsing, command handlers, key access adapters, and reply/error mapping. |
| `src/config/mod.rs` and setting/feature files | Configuration storage, defaults, bounds, validation, update callbacks. |
| `src/filters/mod.rs` and filter files | Command interception, rewrite decisions, and any manually owned filter lifecycle. |
| `src/types/mod.rs` and type-specific files/subdirectories | Custom type registration, encoding, persistence, memory ownership, cleanup. |
| `src/handlers/mod.rs` and handler/feature files | SDK callback adapters and subscriptions: keyspace/server events, client changes, flush DB, config-change notifications. |
| `src/utils/mod.rs` and focused helper files, only if needed | Reusable miscellaneous helpers with no better owner. |
| `tests/` | Server integration tests and their process/connection fixtures. |

### Rust source-file organization

Within each Rust source file, after file-level documentation and imports, use this order:

1. Constants.
2. Traits.
3. Structs, with each struct's corresponding `impl` blocks immediately below it.
4. Enums with each enum's `impl` block immediately below it.
5. Public functions.
6. Private functions.
7. Unit tests at the bottom of the file.
8. Private test helpers below #[test] functions

Integration tests will be in `tests/` folder.

## Version and allocator decisions

Read the resolved SDK's features (SDK source: `Cargo.toml`) and API selection (SDK source: `valkeymodule-rs-macros-internals/src/lib.rs`).
Valkey 9 includes features available Valkey 8, which includes Valkey 7.2 and Redis 7.2, which also includes Redis 7.0.
Redis 6.0 and 6.2 remain separate.
Use `#[cfg(feature = ...)]` to gate features to specific Valkey version
The `use-redismodule-api` feature selects Redis API initialization for Redis targets.
Use `ValkeyAlloc` (SDK source: `src/alloc.rs`) for the normal server module.
Use `test-shims` and `enable-system-alloc` for unit testing

## Testing

### Three-layer test strategy

A feature may need all three layers; passing one layer does not replace the others.

| Layer | Put behavior here | Test approach | What it proves |
| --- | --- | --- | --- |
| Pure Rust unit tests | Parsing, validation, transformations and domain decisions that do not need a Valkey context | Pass ordinary Rust values such as strings, byte slices or small structs to focused functions; test their values and errors directly | Business rules and edge cases independent of the server adapter |
| SDK shim tests | Command, callback and SDK adapter behavior that uses APIs implemented by the resolved SDK's test shims | Invoke the real handler with `create_test_args`, `ValkeyString::test`, and a supported context fixture; assert its returned reply/error or observable Rust state | Argument handling, error mapping and supported SDK calls, not server registration or database behavior |
| Server integration tests | Registration and any behavior the shims do not model, such as key storage, persistence, event delivery, module load/unload and real locking | Load the production module into an isolated server process, issue real commands and assert server-visible effects | The end-to-end contract on the actual target server |

Keep command and callback functions as thin adapters where practical: parse Valkey arguments, call context-free behavior functions, and map their results to Valkey replies.
Put reusable business rules with appropriate files, not in an unstructured `utils/` catchall.
Do not extract code solely to satisfy a test if the resulting boundary obscures ownership.
For pure functions, include normal, boundary and invalid-input cases using ordinary Rust data.
Share fixture setup when it removes real duplication, but keep each test's expected behavior and assertions visible.
Use various SDK example modules to learn how tests call handlers and construct arguments.
Use SDK integration tests as guidance for writing module specific integration tests.

### Reuse test cases and fixtures

When cases exercise the same behavior with the same setup and assertion shape, use a named table of inputs and expected outcomes.
Consider `rstest` `#[case::name]` when separate, identifiable test results make that table easier to maintain; an ordinary loop is sufficient for a small table.
Keep tests separate when their behavior, setup, or expected effects differ.
Extract a helper for repeated setup, or use an `rstest` `#[fixture]` when multiple tests benefit from fixture injection.
Create a fresh `Context::test` or other SDK shim fixture for each case, configure its expectations for that case, and assert the handler's returned value or observable effect.
Do not share a live shim context or server process across unrelated tests merely to reduce setup code.
For server scenarios, put reusable process, connection, and readiness setup in `tests/utils/mod.rs` and keep each scenario's actions and assertions in its test.
Use `tempfile::TempDir` when an isolated server needs temporary data or configuration files, and let the server fixture own the directory and child-process cleanup.
Consider `proptest` for invariants over many generated inputs, such as binary parsing or serialization round trips, rather than replacing named regression cases.
Add these optional crates to the downstream module's `[dev-dependencies]` only when its tests use them; the skill plugin does not supply test crates.

### Test-shim inventory

| Helper | What it actually provides |
| --- | --- |
| `ValkeyString::test` | An owned, binary-safe string from a value implementing `Into<Vec<u8>>`. String shims (SDK source: `src/test-shims/valkey_string.rs`) support reading bytes, numeric parsing, creation/copying, comparison, retain/free. Test non-UTF-8 bytes and embedded NULs where inputs permit them. |
| `test_shims::create_test_args` | Converts a slice of byte-like values (`AsRef<[u8]>`) into owned command arguments. Include the command name at index zero when invoking normal handlers. |
| `Context::test` | Returns an owning `TestContext` that dereferences to `Context`. Keep the fixture alive for the handler call; configure supported APIs through its `expect_*` methods. It is not an in-memory Valkey database. |
| `ThreadSafeContext::test` | Returns `TestThreadSafeContext` for a detached context; configure client ID, calls, or config reads and use `lock`/`with_lock`. It can move to a worker before locking. It does not model a blocked-client context or real server lock contention. |
| `CommandFilterCtx::test` | Returns `TestCommandFilterCtx`; configure `expect_args`, invoke the adapter using `as_raw_ctx_ptr` while the fixture lives, and inspect binary arguments with `args`. Insert/delete operations shift indices. |
| `InfoContext::test` | Returns `TestInfoContext`; invoke an INFO handler and inspect typed, ordered `sections`. `expect_unrequested_section` exercises rejected/unrequested section behavior. |

The exported helper module is `valkey_module::test_shims`.
Its source directory is named test-shims (SDK source: `src/test-shims/mod.rs`).
It is compiled under SDK `cfg(test)` or the `test-shims` feature.
A downstream crate's unit tests do **not** turn on SDK `cfg(test)`; enable `test-shims` on a development dependency using the same SDK source/version as the normal dependency.
Keep shims and the system allocator out of the production module artifact.
Do not load a shim-enabled module into a server.
`Context::dummy` is a null context, not a substitute for `Context::test` when code uses context APIs.

Read context fixtures (SDK source: `src/test-shims/context.rs`) before testing a context-dependent handler.
These configure responses; they do not execute commands or validate server configuration registration.
Supported methods include:

- `expect_get_client_id`, `expect_get_client_name_by_id`, `expect_set_client_name_by_id`, `expect_get_client_username_by_id`, `expect_get_client_cert`, `expect_get_client_info_by_id`, and `expect_get_client_ip_by_id`.
  The info expectation takes `RedisModuleClientInfo` and derives the ID from that structure.
- `expect_get_current_user`, `expect_authenticate_client_with_acl_user`, `expect_deauthenticate_and_close_client_by_id`, and `expect_get_server_version` for the corresponding context lookups/actions.
- `expect_config_get` for a configured config read and `expect_call` for an exact command byte string and exact argument bytes.
  `expect_call` matches case-sensitively, searches the first matching expectation, and reuses it; it is not a consumed sequence or call-count assertion.

### Tntegration tests

Keep server integration tests in `tests/`, using the downstream project's existing harness.
Follow the SDK's separation of feature areas from reusable process helpers, while organizing downstream tests by capability when the suite grows:

```text
tests/
  integration.rs          # integration test entry point; declares mod utils and scenario modules
  utils/
    mod.rs                # shared server fixture and test helpers
```

Put only genuinely shared harness code in `tests/utils/mod.rs`; split that module into focused helper files only when its responsibilities grow.
Build and load the production module artifact with the features intended for the target server.
SDK harness files (SDK source: `tests/utils/mod.rs`), integration tests (`tests/integration.rs`), engine matrix (`integration-servers.conf`), and test driver (`test.sh`) are evidence for design, not exported utilities a downstream crate can import.
SDK aliases in `.cargo/config.toml` apply in this checkout only.

## Implementation verification

At the end of every downstream implementation task using `create-module`, `add-command`, `add-config`, `add-data-type`, `add-event-handler`, `add-filter`, or `add-module-lifecycle`, run these commands from the module crate root in this order:

```sh
cargo fmt
cargo check
cargo build
cargo test
```

Check for compile errors, then build before testing so real-server tests can load a fresh production module artifact; a clean `cargo test` alone may not produce the loadable library.
Inspect formatting changes and preserve unrelated edits.
Run any additional targeted checks or server scenarios the feature needs.
If a command cannot run or fails, report that outcome and its prerequisite or failure; do not claim verification passed.
Record exact commands, outcomes, warnings, skipped checks, and missing prerequisites.
