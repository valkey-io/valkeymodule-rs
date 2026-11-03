---
name: create-module
description: Create a minimal downstream Rust Valkey module crate with a loadable cdylib, registration skeleton, README, and integration-test entry point. Use for a new module project, not SDK implementation changes.
---

# Create a Valkey module skeleton

Follow [shared architecture and testing](../../references/module-architecture-and-testing.md).
Do not invent a module name, destination, or permission to overwrite existing files.

## Wizard

| Item | Resolve |
| --- | --- |
| Name | What is the module name? It must be explicit; use it consistently as the Cargo package name, module name, README heading, and artifact-name basis. |
| Destination | What exact directory should contain the new crate? |
| SDK dependency | Which `valkey-module` dependency source should the crate use: the latest published crate, or an explicitly supplied local/Git SDK source? |
| Existing destination | Ask only when the destination already contains files: may this skill create its skeleton files there without overwriting unrelated content? If the requested skeleton file already exists, obtain an explicit overwrite decision before replacing it. |

Use the table as the internal checklist.
The Name item is always applicable; apply the shared [explicit-name requirement](../../references/module-architecture-and-testing.md#require-an-explicit-name-before-implementation).
This skill creates an empty module skeleton only.
It has no commands, filters, configurations, data types, or event handlers: do not ask the user to design placeholder behavior.
Do not require a Valkey/Redis version or target platform unless the request supplies a compatibility constraint.
Create exactly `Cargo.toml`, `src/lib.rs`, `.gitignore`, `README.md`, `tests/utils/mod.rs`, and `tests/integration.rs`.
Add behavior later only when requested, using [add-command](../add-command/SKILL.md), [add-config](../add-config/SKILL.md), [add-data-type](../add-data-type/SKILL.md), [add-filter](../add-filter/SKILL.md), [add-event-handler](../add-event-handler/SKILL.md), or [add-module-lifecycle](../add-module-lifecycle/SKILL.md).

## Code samples

Adapt these files to the confirmed [module contract](#wizard).

### `Cargo.toml`

The package's own `version` is separate from the SDK dependency version.
Check for more recent versions of various dependencies and use them if available.

```toml
[package]
name = "my-module"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
valkey-module = "0.1.14"

[dev-dependencies]
valkey-module = { version = "0.1.14", features = ["test-shims"] }
anyhow = "1"
redis = "0.28"
rstest = "0.27"
tempfile = "3"
```

Keep `Cargo.lock` tracked for reproducible builds; do not add it to `.gitignore`.

### `src/lib.rs`

Replace `my-module` with the requested module name.
Keep the macro registration empty until a feature-specific skill adds behavior.

```rust
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::valkey_module;

valkey_module! {
    name: "my-module",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [],
}
```

### `.gitignore`

Ignore generated build output and common local editor/OS clutter only.

```gitignore
/target/
/.DS_Store
*.rdb
.codegraph/
```

### `README.md`

Create an otherwise empty README containing only the requested module name as its heading.

```markdown
# my-module
```

### `tests/utils/mod.rs`

Create a small real-server fixture, adapting the SDK integration-test pattern.
It owns an isolated temporary data directory and child process; chooses a random available loopback port; starts `valkey-server` with the production cdylib passed through `--loadmodule`; waits for a client connection with a finite deadline; and sends `SHUTDOWN NOSAVE`, then kills the child only after a bounded shutdown timeout.
Locate the cdylib in `target/debug` (or `target/release` when running release tests), canonicalize its path before changing to the temporary data directory, and use `lib{module_name}.dylib` on macOS or `lib{module_name}.so` elsewhere.
Pass `&[]` to `start` when the module needs no load-time arguments.

```rust
use anyhow::{Context, Result};
use redis::Connection;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

const READY_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(super) struct TestServer {
    child: Child,
    _d: TempDir,
    port: u16,
}

impl TestServer {
    pub(super) fn start(module_name: &str, module_args: &[&str]) -> Result<Self> {
        let port = TcpListener::bind("127.0.0.1:0")?.local_addr()?.port();
        let data_dir = tempfile::tempdir().context("create server data directory")?;
        let module_path = module_path(module_name)?;
        let child = Command::new("valkey-server")
            .arg("--port")
            .arg(port.to_string())
            .arg("--dir")
            .arg(data_dir.path())
            .args(["--save", "", "--appendonly", "no"])
            .arg("--loadmodule")
            .arg(&module_path)
            .args(module_args)
            .current_dir(data_dir.path())
            .spawn()
            .context("start valkey-server")?;

        let server = Self {
            child,
            _d: data_dir,
            port,
        };
        server.connection()?;
        Ok(server)
    }

    pub(super) fn connection(&self) -> Result<Connection> {
        let client = redis::Client::open(format!("redis://127.0.0.1:{}/", self.port))?;
        let deadline = Instant::now() + READY_TIMEOUT;
        loop {
            match client.get_connection() {
                Ok(connection) => return Ok(connection),
                Err(error) if error.is_connection_refusal() && Instant::now() < deadline => {
                    thread::sleep(POLL_INTERVAL);
                }
                Err(error) => return Err(error).context("connect to server"),
            }
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Ok(mut connection) = self.connection() {
            let _: redis::RedisResult<()> =
                redis::cmd("SHUTDOWN").arg("NOSAVE").query(&mut connection);
        }
        let deadline = Instant::now() + READY_TIMEOUT;
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            thread::sleep(POLL_INTERVAL);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn module_path(module_name: &str) -> Result<PathBuf> {
    let extension = if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };
    let profile = if cfg!(not(debug_assertions)) {
        "release"
    } else {
        "debug"
    };
    let artifact_name = module_name.replace('-', "_");
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(profile)
        .join(format!("lib{artifact_name}.{extension}"));
    path.canonicalize()
        .with_context(|| format!("module not found: {}", path.display()))
}
```

### `tests/integration.rs`

Keep server lifecycle mechanics in `utils/mod.rs`.
The integration test starts the fixture and verifies that Valkey reports the module as loaded.

```rust
mod utils;

use anyhow::Result;
use redis::Value;
use utils::TestServer;

#[test]
fn loads_the_module() -> Result<()> {
    let server = TestServer::start("my-module", &[])?;
    let mut connection = server.connection()?;
    let modules: Vec<Value> = redis::cmd("MODULE").arg("LIST").query(&mut connection)?;

    assert!(format!("{modules:?}").contains("my-module"));
    Ok(())
}
```

## Implementation

Create the skeleton files from the [code samples](#code-samples) using the confirmed module name, destination, and SDK dependency.
Report failures rather than claiming the skeleton is verified.
After verification, initialize a Git repository at the new crate root with `git init` only if the destination is not already inside a Git repository.
Do not run `git add` or `git commit`.
Follow the shared [testing guidance](../../references/module-architecture-and-testing.md#testing) for unit, shim, and server coverage.
Finish by reporting the shared [implementation verification](../../references/module-architecture-and-testing.md#implementation-verification) results and any remaining limits.
