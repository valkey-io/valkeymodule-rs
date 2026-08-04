use anyhow::{Context, Result};

use redis::Connection;
use redis::RedisResult;
use std::fs;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

const SHUTDOWN_CONNECTION_TIMEOUT: Duration = Duration::from_millis(250);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Owns a Valkey test process and the connection used to communicate with it.
pub(super) struct TestServer {
    pub(super) port: u16,
    _guard: ChildGuard,
    connection: Connection,
}

// Allows TestServer to be used where an immutable Redis connection is expected.
impl Deref for TestServer {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

// Allows TestServer to be used where a mutable Redis connection is expected.
impl DerefMut for TestServer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}

// Exposes only the lifecycle operations that integration tests need.
impl TestServer {
    pub(super) fn data_dir(&self) -> &std::path::Path {
        self._guard.data_dir()
    }

    pub(super) fn into_parts(self) -> (ChildGuard, Connection) {
        (self._guard, self.connection)
    }
}

/// Shuts down the child Valkey process and removes its temporary data directory on drop.
pub(super) struct ChildGuard {
    name: &'static str,
    port: u16,
    data_dir: PathBuf,
    child: std::process::Child,
}

// Gracefully stops the child process before removing its isolated data directory.
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let client = redis::Client::open(format!("redis://127.0.0.1:{}/", self.port));
        if let Ok(client) = client {
            if let Ok(mut connection) =
                client.get_connection_with_timeout(SHUTDOWN_CONNECTION_TIMEOUT)
            {
                let _: RedisResult<()> =
                    redis::cmd("SHUTDOWN").arg("NOSAVE").query(&mut connection);
            }
        }

        let shutdown_deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => {
                    self.cleanup_data_dir();
                    return;
                }
                Ok(None) if Instant::now() < shutdown_deadline => {
                    std::thread::sleep(SHUTDOWN_POLL_INTERVAL);
                }
                Ok(None) => {
                    if let Err(e) = self.child.kill() {
                        println!("Could not kill {} after shutdown timeout: {e}", self.name);
                    }
                    if let Err(e) = self.child.wait() {
                        println!(
                            "Could not wait for {} after shutdown timeout: {e}",
                            self.name
                        );
                    }
                    self.cleanup_data_dir();
                    return;
                }
                Err(e) => {
                    println!("Could not check whether {} exited: {e}", self.name);
                    return;
                }
            }
        }
    }
}

// Contains cleanup behavior used by the child process lifecycle.
impl ChildGuard {
    fn data_dir(&self) -> &std::path::Path {
        &self.data_dir
    }

    fn cleanup_data_dir(&self) {
        if let Err(e) = fs::remove_dir_all(&self.data_dir) {
            println!("Could not remove Valkey data directory: {e}");
        }
    }
}

pub(super) fn start_server_w_module_get_connection(module_name: &str) -> Result<TestServer> {
    let port = get_available_port()?;
    let guard = start_valkey_server_with_module(module_name, port)
        .with_context(|| "failed to start valkey server")?;
    let connection =
        get_valkey_connection(port).with_context(|| "failed to connect to valkey server")?;

    Ok(TestServer {
        port,
        _guard: guard,
        connection,
    })
}

fn start_valkey_server_with_module(module_name: &str, port: u16) -> Result<ChildGuard> {
    let module_path = get_module_path(module_name)?;
    let data_dir =
        std::env::temp_dir().join(format!("valkeymodule-rs-{}-{port}", std::process::id()));
    fs::create_dir(&data_dir)?;
    let port_arg = port.to_string();
    let data_dir_arg = data_dir
        .to_str()
        .context("Valkey data directory is not valid UTF-8")?;

    let args = &[
        "--port",
        port_arg.as_str(),
        "--dir",
        data_dir_arg,
        "--dbfilename",
        "dump.rdb",
        "--loadmodule",
        module_path.as_str(),
        "--enable-debug-command",
        "yes",
        "--enable-module-command",
        "yes",
    ];

    let child = Command::new("valkey-server")
        .args(args)
        .current_dir(&data_dir)
        .spawn();
    let child = match child {
        Ok(child) => child,
        Err(error) => {
            let _ = fs::remove_dir_all(&data_dir);
            return Err(error.into());
        }
    };

    Ok(ChildGuard {
        name: "server",
        port,
        data_dir,
        child,
    })
}

fn get_available_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

pub(super) fn get_module_path(module_name: &str) -> Result<String> {
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
pub(super) fn get_valkey_connection(port: u16) -> Result<Connection> {
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
pub(super) enum AuthExpectedResult {
    Success,
    Denied,
    EngineDenied,
    Aborted,
}

// Helper function to validate the authentication
pub(super) fn check_auth(
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

pub(super) fn setup_acl_users(
    con: &mut redis::Connection,
    users: &[(&str, Option<&str>)],
) -> Result<()> {
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

pub(super) fn check_blocked_clients(con: &mut redis::Connection) -> Result<i32> {
    let info: String = redis::cmd("INFO").arg("clients").query(con)?;

    let blocked_clients = info
        .lines()
        .find(|line| line.starts_with("blocked_clients:"))
        .and_then(|line| line.split(':').nth(1))
        .and_then(|count| count.trim().parse::<i32>().ok())
        .unwrap_or(0);

    Ok(blocked_clients)
}
