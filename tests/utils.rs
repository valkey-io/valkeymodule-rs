use anyhow::{Context, Result};

use redis::Connection;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

pub struct ChildGuard {
    name: &'static str,
    child: std::process::Child,
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Err(e) = self.child.kill() {
            println!("Could not kill {}: {e}", self.name);
        }
        if let Err(e) = self.child.wait() {
            println!("Could not wait for {}: {e}", self.name);
        }
    }
}

pub fn start_valkey_server_with_module(module_name: &str, port: u16) -> Result<ChildGuard> {
    let module_path = get_module_path(module_name)?;

    let args = &[
        "--port",
        &port.to_string(),
        "--loadmodule",
        module_path.as_str(),
        "--enable-debug-command",
        "yes",
        "--enable-module-command",
        "yes",
    ];

    let valkey_server = Command::new("valkey-server")
        .args(args)
        .spawn()
        .map(|c| ChildGuard {
            name: "server",
            child: c,
        })?;

    Ok(valkey_server)
}

pub(crate) fn get_module_path(module_name: &str) -> Result<String> {
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

    let module_path: PathBuf = [
        std::env::current_dir()?,
        PathBuf::from(format!(
            "target/{profile}/examples/lib{module_name}.{extension}"
        )),
    ]
    .iter()
    .collect();

    assert!(fs::metadata(&module_path)
        .with_context(|| format!("Loading valkey module: {}", module_path.display()))?
        .is_file());

    let module_path = format!("{}", module_path.display());
    Ok(module_path)
}

// Get connection to Redis
pub fn get_valkey_connection(port: u16) -> Result<Connection> {
    let client = redis::Client::open(format!("redis://127.0.0.1:{port}/"))?;
    loop {
        let res = client.get_connection();
        match res {
            Ok(con) => return Ok(con),
            Err(e) => {
                if e.is_connection_refusal() {
                    // Valkey not ready yet, sleep and retry
                    std::thread::sleep(Duration::from_millis(50));
                } else {
                    return Err(e.into());
                }
            }
        }
    }
}
