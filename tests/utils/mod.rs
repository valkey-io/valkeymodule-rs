use anyhow::{Context, Result};

use redis::Connection;
use redis::RedisResult;
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

#[derive(Debug)]
pub enum AuthExpectedResult {
    Success,
    Denied,
    EngineDenied,
    Aborted,
}

// Helper function to validate the authentication
pub fn check_auth(
    con: &mut redis::Connection,
    username: &str,
    password: &str,
    expected_result: AuthExpectedResult,
) -> Result<()> {
    let response: RedisResult<String> = redis::cmd("AUTH").arg(&[username, password]).query(con);

    match expected_result {
        AuthExpectedResult::Success => {
            let res =
                response.with_context(|| format!("failed to authenticate {username} user"))?;
            assert_eq!(res, "OK");
        }
        AuthExpectedResult::Denied => {
            assert!(response.is_err());
            let err = response.unwrap_err().to_string();
            assert!(
                err.contains("DENIED: Authentication credentials mismatch"),
                "Unexpected error message: {}",
                err
            );
        }
        AuthExpectedResult::EngineDenied => {
            assert!(response.is_err());
            let err = response.unwrap_err().to_string();
            assert!(
                err.contains("WRONGPASS: invalid username-password pair or user is disabled"),
                "Unexpected error message: {}",
                err
            );
        }
        AuthExpectedResult::Aborted => {
            assert!(response.is_err());
            let err = response.unwrap_err().to_string();
            assert!(
                err.contains("ABORT: Authentication aborted by server"),
                "Unexpected error message: {}",
                err
            );
        }
    }
    Ok(())
}

pub fn setup_acl_users(con: &mut redis::Connection, users: &[(&str, Option<&str>)]) -> Result<()> {
    for (user, maybe_pass) in users {
        let res: String = if let Some(pass) = maybe_pass {
            redis::cmd("ACL")
                .arg(&["SETUSER", user, "on", &format!(">{}", pass), "~*", "+@all"])
                .query(con)?
        } else {
            redis::cmd("ACL")
                .arg(&["SETUSER", user, "on", "nopass", "~*", "+@all"])
                .query(con)?
        };
        assert_eq!(&res, "OK");
    }
    Ok(())
}

pub fn check_blocked_clients(con: &mut redis::Connection) -> Result<i32> {
    let info: String = redis::cmd("INFO").arg("clients").query(con)?;

    let blocked_clients = info
        .lines()
        .find(|line| line.starts_with("blocked_clients:"))
        .and_then(|line| line.split(':').nth(1))
        .and_then(|count| count.trim().parse::<i32>().ok())
        .unwrap_or(0);

    Ok(blocked_clients)
}
