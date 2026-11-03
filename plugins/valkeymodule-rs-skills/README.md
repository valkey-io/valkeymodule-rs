# valkeymodule-rs-skills

AI skills for building and extending well-structured Valkey modules with the valkeymodule-rs Rust SDK.

`valkeymodule-rs-skills` marketplace currently lists one plugin `valkeymodule-rs-skills`.
It installs several workflows to create Valkey Modules using Rust SDK.
It does not install or compile Valkey, Rust tooling or other dependencies.
After adding these skills to Codex environment you can run them to create new or modify existing modules.

## Package layout

The SDK repository is the marketplace root.
Its catalog is `.agents/plugins/marketplace.json`; that catalog points to `./plugins/valkeymodule-rs-skills`.
Paths resolve from the repository root, not from `.agents/plugins/`.

```text
plugins/valkeymodule-rs-skills/
  plugin.json
  .codex-plugin/plugin.json
  README.md
  skills/
    create-module/SKILL.md
    add-command/SKILL.md
    add-config/SKILL.md
    add-filter/SKILL.md
    add-data-type/SKILL.md
    add-event-handler/SKILL.md
    add-module-lifecycle/SKILL.md
    test-and-review-module/SKILL.md
  references/
    module-architecture-and-testing.md
```

The root manifest uses the portable Agent Plugins format; the matching `.codex-plugin/plugin.json` supplies Codex-compatible discovery and display metadata.
This follows the [official packaging documentation](https://developers.openai.com/plugins/build/plugins).
Keep the name/version/description in both manifests aligned when releasing a new plugin revision.
The SDK source is deliberately outside the installed package.

## Installation

Clone this repo and follow instructions to install plugin for your Codex environment.
```sh
git clone https://github.com/dmitrypol/valkeymodule-rs.git
cd valkeymodule-rs
codex plugin marketplace add .
codex plugin marketplace list
codex plugin add valkeymodule-rs-skills@valkeymodule-rs-skills
codex plugin list
codex plugin remove valkeymodule-rs-skills@valkeymodule-rs-skills
```
You might need to restart the Codex app.
`AVAILABLE` in the catalog means installation is optional, not automatic.

## Invoke a workflow in your module project

Use Codex's skill picker to select the workflow under **valkeymodule-rs-skills**, or mention its skill name explicitly.
If another plugin exposes the same short name, choose this plugin's entry in the picker.
Each workflow is installed as part of the single plugin.

| Workflow | Request it handles                                              |
| --- |-----------------------------------------------------------------|
| [create-module](skills/create-module/SKILL.md) | Start a downstream module with proper code structure and tests. |
| [add-command](skills/add-command/SKILL.md) | Add Valkey command                                              |
| [add-config](skills/add-config/SKILL.md) | Add module configuration                                        |
| [add-filter](skills/add-filter/SKILL.md) | Intercept/rewrite commands and verify filter lifecycle.         |
| [add-data-type](skills/add-data-type/SKILL.md) | Add custom data structures.                                     |
| [add-event-handler](skills/add-event-handler/SKILL.md) | Add server-event handlers and their tests. |
| [add-module-lifecycle](skills/add-module-lifecycle/SKILL.md) | Add preload, init, and deinit hooks in `src/lifecycle/mod.rs`. |
| [test-and-review-module](skills/test-and-review-module/SKILL.md) | Review structure and verify unit/server behavior.               |

For example, ask to use `add-command` to add the desired command to the current modules.
Or ask `test-and-review-module` to review the current module without changing it.
The skill directs edits to your module, not the SDK or example modules.

- For a new module: “Use create-module from valkeymodule-rs-skills to create my module in the destination I specify.
    Use my local SDK checkout as the API reference, keep registration small, and plan unit and server tests.”
- For an existing module: choose the focused workflow for the requested change, such as `add-command`, `add-config`, `add-data-type`, `add-filter`, `add-event-handler`, or `add-module-lifecycle`.
    Preserve unrelated behavior and extract only touched logic from `lib.rs` into focused module directories when needed.
- For a focused change: “Use add-event-handler to track client connection changes.
    Put the adapter in handlers/ and identify which tests require a real server.”
- For review only: “Use test-and-review-module to review this module's structure, ownership and tests.
    Report findings without changing implementation files.”

Provide the local SDK clone path in the task.
Existing projects resolve their actual dependency source from Cargo metadata; no absolute developer-machine path is hardcoded in the plugin, and no environment variable or global setup is required for SDK discovery.

The [shared architecture, SDK-resolution and testing guidance](references/module-architecture-and-testing.md) explains how to verify the SDK source, keep `lib.rs` small, use focused capability directories and callback adapters, and distinguish supported shim-based unit tests from behavior requiring a real server.
`utils/` and background workers are added only when needed.

## Updating and checking an installation

Git pull the repo.
Remove and add the plugin
You might need to restart Codex app.
