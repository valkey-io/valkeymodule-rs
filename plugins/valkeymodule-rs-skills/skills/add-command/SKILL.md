---
name: add-command
description: Add or modify a command in a downstream valkeymodule-rs module, including argument parsing, replies, key metadata, and handler tests. Use for client commands rather than filters or event subscriptions.
---

# Add or change a command

Read [shared architecture and testing](../../references/module-architecture-and-testing.md), then inspect the module's existing registration style.
Inspect examples modules in this SDK for various commands.

## Wizard

Follow the shared [wizard protocol](../../references/module-architecture-and-testing.md#wizard-protocol) using this command-contract table.

| Item | Resolve |
| --- | --- |
| Name | What is the public command name? It must be explicit; do not derive or invent one. |
| Arguments | What is the required and optional user-argument grammar, including options, repeated groups, and numeric bounds? State whether each value is arbitrary bytes or text and, for text, its encoding and validation rules. Argument zero is always the command name. |
| Keys | Which argument positions are keys, and what access does each need (`read`, `write`, or both)? For variable or movable keys, state the discovery rule and cluster requirements. If there are no keys, say so explicitly. |
| Replies and errors | What exact success reply shape and error cases should clients receive? Include wrong arity, malformed input, bounds, missing keys, wrong key types, and conflict behavior when applicable. |
| Effects | Does the command read state only, mutate state, call other commands, block, publish, or have another visible side effect? State the behavior for existing values and partial failures. |
| Replication | Ask only for mutations or replicated calls: what must replicas and AOF receive, and should the handler use verbatim replication, explicit `replicate`, or a call option? |
| Registration metadata | What command flags, key metadata, arity, and ACL categories apply? Resolve them against the registration mechanism already used by the module. The entry macro needs flag text plus first/last/step key positions; an attribute command needs its corresponding metadata. |
| Server target | What is the minimum Valkey/Redis server target or SDK feature requirement, especially for metadata, key discovery, blocking, or replication APIs? |

Use the table as the internal checklist.
The Name item is always applicable.
Apply the shared [explicit-name requirement](../../references/module-architecture-and-testing.md#require-an-explicit-name-before-implementation): do not invent a public name.
If no command flags, keys, ACL categories, or replication behavior apply, say so explicitly rather than silently assuming them.
Treat a read-only command as non-mutating; do not separately ask for replication.
For a command with no keys, use zero key positions rather than asking for a key-discovery rule.

## Code samples

Use a named stub when starting a new command.
This `my-command` example requires one user argument (after its name at argument zero), logs a notice, and returns that argument unchanged.

### `src/commands/my_command.rs`

```rust
use valkey_module::{Context, ValkeyError, ValkeyResult, ValkeyString};

pub(crate) fn my_command(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() != 2 {
        return Err(ValkeyError::WrongArity);
    }

    let value = args
        .into_iter()
        .nth(1)
        .expect("argument count checked above");
    ctx.log_notice("my-command received an argument");
    Ok(value.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use valkey_module::test_shims::create_test_args;
    use valkey_module::ValkeyValue;

    #[test]
    fn returns_first_argument_without_changing_its_bytes() {
        let args = create_test_args(&[b"my-command".as_slice(), b"\0\xff".as_slice()]);
        let result =
            my_command(&Context::dummy(), args).expect("command with one argument should succeed");

        assert!(matches!(
            result,
            ValkeyValue::BulkValkeyString(value) if value.as_slice() == b"\0\xff"
        ));
    }

    #[test]
    fn rejects_wrong_arity() {
        for args in [&["my-command"][..], &["my-command", "value", "extra"][..]] {
            let result = my_command(&Context::dummy(), create_test_args(args));

            assert!(matches!(result, Err(ValkeyError::WrongArity)));
        }
    }
}
```

### `src/commands/mod.rs`

```rust
mod my_command;

pub(crate) use my_command::my_command;
```

### Register in `src/lib.rs`

In the existing `src/lib.rs`, declare `mod commands;`, import `commands::my_command`, and replace `commands: []` with `commands: [["my-command", my_command, "", 0, 0, 0]],`.
The empty flags and zero key positions match this no-key example.

### Integration test sample

Add this test to the existing `tests/integration.rs` file.
Keep its module-load test and `mod utils;` declaration.
The `create-module` skeleton already imports `Result` and `TestServer` there.
Replace `my-module` with the downstream module name in the test.

```rust
#[test]
fn my_command_returns_first_argument() -> Result<()> {
    let server = TestServer::start("my-module")?;
    let mut connection = server.connection()?;
    let reply: String = redis::cmd("my-command")
        .arg("hello")
        .query(&mut connection)?;

    assert_eq!(reply, "hello");
    Ok(())
}
```

## Implementation

1. Resolve the [command contract](#wizard) before writing any code.
2. Start with the [code samples](#code-samples) when a basic handler and test skeleton is useful.
   Rename `my_command` to the intended name, replace its exact-one-argument check and echoed reply with the command's specified contract, then add only the context interaction and argument parsing the command needs.
   Keep the focused success and error tests, updating their inputs and assertions with the implementation.
3. Assign parsing/handler/reply mapping to a command or cohesive feature file under `commands/`; keep its `mod.rs` to declarations/re-exports.
   Leave registration wiring in `lib.rs`.
4. The entry macro's command tuples provide name, handler, flags, and first/last/step key positions.
   Do not register the same command twice.
5. Validate required and extra arguments, numeric bounds/overflow, malformed options, and wrong key types before mutation.
   Use `ValkeyError::WrongArity`, `NextArg` and appropriate parsing APIs.
   Return the intended `ValkeyResult` reply.
6. Follow the shared [testing guidance](../../references/module-architecture-and-testing.md#testing) and unit-test the handler's returned value/error with `create_test_args` or `ValkeyString::test`, including command name, invalid inputs and boundary cases.
   Use `Context::test` only for supported calls; exact `expect_call` responses do not simulate key storage or replication.
   Write appropriate integration tests to actually call commmands and verify results.
7Finish with the shared [implementation verification](../../references/module-architecture-and-testing.md#implementation-verification) sequence, focused command tests and any applicable server coverage.
   Report the commands run, their outcomes, and unsupported paths that still need server coverage.
