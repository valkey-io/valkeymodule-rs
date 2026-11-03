---
name: test-and-review-module
description: Test or review a downstream valkeymodule-rs module for behavior, structure, SDK ownership and lifecycle correctness, using supported test shims and real-server checks. Use for module verification; a review request does not authorize fixes.
---

# Test and review a module

Read [shared architecture and testing](../../references/module-architecture-and-testing.md) in full.
It defines the structure check, downstream feature setup, exact helper capabilities and limits.
Distinguish requested implementation/testing from a read-only review; do not modify production code for a review-only request.

## Wizard

Follow the shared [wizard protocol](../../references/module-architecture-and-testing.md#wizard-protocol) using this review-and-testing contract table.
For review-only work, resolve scope from the request and available evidence; the implementation confirmation gate applies only when code changes are requested.

| Item | Resolve |
| --- | --- |
| Scope | Which module, change, or behavior should be reviewed or tested? |
| Authorized work | Is the request review-only, execution of existing tests, or permission to add tests or implement fixes? |
| Compatibility | Which resolved SDK features and server targets must the checks cover? |
| Evidence | Which existing test harness and available server fixtures can verify the requested behavior? |

## Code samples

Use the SDK's handler tests (SDK source: `examples/hello.rs`, `examples/subcmd.rs`) and supported fixtures (SDK source: `src/test-shims/mod.rs`) as references when adding tests is authorized.
Adapt assertions to the downstream behavior and use the shared [testing guidance](../../references/module-architecture-and-testing.md#testing) to select unit, shim, or real-server coverage.

## Implementation

For review-only requests, these steps produce findings and verification results without modifying production code.

1. Identify the review scope, existing changes, resolved SDK source, feature selections, supported servers and test harness.
   Map entry-point registration to behavior files and state owners.
   For a version change, inspect API feature selection (SDK source: `valkeymodule-rs-macros-internals/src/lib.rs`) and version map (SDK source: `valkeymodule-rs-macros-internals/src/api_versions.rs`), not only the manifest.
2. Review structure and identity: apply the shared explicit-name requirement for module, command, native type, and filter work, checking the chosen name against Cargo metadata, Rust declarations, and Valkey registration/descriptor as applicable.
   `src/lib.rs` declares modules and registers callbacks; `src/lifecycle/mod.rs` contains module `preload`, `init`, and `deinit` hooks and their unit tests.
   Capability directories have small `mod.rs` files and focused command/filter/handler/feature files.
   Tests follow behavior and server tests live in `tests/`.
   Flag concrete readability/ownership problems and propose focused extractions of touched behavior; preserve coherent alternatives rather than enforcing a wholesale rewrite.
   This is a skill-guided check, not an installed automated rule.
3. Require the three-layer test structure from the shared reference: context-free business logic gets ordinary Rust unit tests; command/callback adapters use only supported SDK shims; server-only behavior gets isolated integration tests.
   Confirm that tests exercise meaningful boundaries at each layer rather than treating broad integration coverage as a substitute for focused unit tests.
4. If authorized to add tests, exercise real downstream handlers and state transitions.
   Include wrong arity, extra/malformed/binary input, domain boundaries, error replies, and changed state as applicable.
   Use SDK argument/context fixtures when supported; expectations configure inputs, not assertions: `expect_call` is reusable exact-match lookup and has no call-count/consumption verification.
   Assert meaningful outputs and observable state.
   Inspect local helper implementation (SDK source: `src/test-shims/mod.rs`) and example tests (SDK source: `examples/hello.rs`, `examples/subcmd.rs`) for supported use, not new global mocks.
5. Review test organization: behavior tests stay beside their implementation; server scenarios live under `tests/` with `tests/integration.rs` as the suite entry point and shared process/connection/readiness helpers in `tests/utils/mod.rs`.
   Keep assertions with scenario tests, extract helpers only when shared, isolate each server fixture, bound retries and waits, and support explicit configuration and module lifecycle operations when needed.
   Adapt SDK harness patterns; do not assume its private `tests/utils` code is a downstream library.
6. Review SDK invariants: registration uniqueness and macro identifier visibility; production allocator versus test features; feature-dependent signatures; correct key flags; replication/persistence decisions; callback restrictions; string/key/reference lifetime; raw-pointer ownership; filter removal; configuration rejection state; worker shutdown and lock ordering.
   Follow specialized workflows when a path needs more detail, especially [configuration](../add-config/SKILL.md), [native types](../add-data-type/SKILL.md), [events](../add-event-handler/SKILL.md), and [module lifecycle](../add-module-lifecycle/SKILL.md).
7. Run the downstream project's relevant debug unit tests and production build/check, then applicable isolated server integration tests.
   Verify that the loaded artifact excludes shims and uses features matching the target server.
   SDK `cargo test-shims` and `test.sh` are repository-local workflows, not universal downstream commands.
   Missing servers/dependencies are verification limits, not a passing result; do not install or configure unrelated tools implicitly.
8. Report actionable findings with file locations, concrete trigger, impact and a focused correction.
   Separately report checks performed, outcomes, warnings and gaps.
   For completed implementation, summarize file placement and tested behavior; for review, distinguish demonstrated defects from questions requiring server evidence.
   Do not claim memory, concurrency or persistence correctness from shim tests alone.

Finish with the applicable shared [implementation verification](../../references/module-architecture-and-testing.md#implementation-verification) checks and report their results.
