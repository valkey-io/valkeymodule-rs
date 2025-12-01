use std::thread;
use std::time::Duration;

use crate::utils::get_module_path;
use anyhow::Context;
use anyhow::Result;
use redis::Commands;
use redis::Value;
use redis::{RedisError, RedisResult};
use utils::{
    check_auth, check_blocked_clients, get_valkey_connection, setup_acl_users,
    start_valkey_server_with_module, AuthExpectedResult,
};

const FAILED_TO_START_SERVER: &str = "failed to start valkey server";
const FAILED_TO_CONNECT_TO_SERVER: &str = "failed to connect to valkey server";

mod utils;

#[test]
fn test_hello() -> Result<()> {
    let port: u16 = 6479;
    let _guards =
        vec![start_valkey_server_with_module("hello", port)
            .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    let res: Vec<i32> = redis::cmd("hello.mul")
        .arg(&[3, 4])
        .query(&mut con)
        .with_context(|| "failed to run hello.mul")?;
    assert_eq!(res, vec![3, 4, 12]);

    let res: Result<Vec<i32>, RedisError> =
        redis::cmd("hello.mul").arg(&["3", "xx"]).query(&mut con);
    if res.is_ok() {
        return Err(anyhow::Error::msg("Should return an error"));
    }

    Ok(())
}

#[test]
fn test_keys_pos() -> Result<()> {
    let port: u16 = 6480;
    let _guards = vec![start_valkey_server_with_module("keys_pos", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    let res: Vec<String> = redis::cmd("keys_pos")
        .arg(&["a", "1", "b", "2"])
        .query(&mut con)
        .with_context(|| "failed to run keys_pos")?;
    assert_eq!(res, vec!["a", "b"]);

    let res: Result<Vec<String>, RedisError> =
        redis::cmd("keys_pos").arg(&["a", "1", "b"]).query(&mut con);
    if res.is_ok() {
        return Err(anyhow::Error::msg("Should return an error"));
    }

    Ok(())
}

#[test]
fn test_helper_version() -> Result<()> {
    let port: u16 = 6481;
    let _guards = vec![start_valkey_server_with_module("test_helper", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    let res: Vec<i64> = redis::cmd("test_helper.version")
        .query(&mut con)
        .with_context(|| "failed to run test_helper.version")?;
    assert!(res[0] > 0);

    // Test also an internal implementation that might not always be reached
    // TODO: this check is currently disabled because Valkey 8.0.0 returns
    //       redis_version:7.2.4 and the test expects it to be 8.0.0
    // let res2: Vec<i64> = redis::cmd("test_helper._version_rm_call")
    //     .query(&mut con)
    //     .with_context(|| "failed to run test_helper._version_rm_call")?;
    // assert_eq!(res, res2);

    let res3: String = redis::cmd("test_helper.name")
        .query(&mut con)
        .with_context(|| "failed to run test_helper.name")?;
    assert_eq!(res3, "test_helper.name");

    Ok(())
}

#[test]
fn test_command_name() -> Result<()> {
    use valkey_module::ValkeyValue;

    let port: u16 = 6482;
    let _guards = vec![start_valkey_server_with_module("test_helper", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    // Call the tested command
    let res: Result<String, RedisError> = redis::cmd("test_helper.name").query(&mut con);

    // The expected result is according to valkey version
    let info: String = redis::cmd("info")
        .arg(&["server"])
        .query(&mut con)
        .with_context(|| "failed to run test_helper.name")?;

    if let Ok(ver) = valkey_module::Context::version_from_info(ValkeyValue::SimpleString(info)) {
        if ver.major > 6
            || (ver.major == 6 && ver.minor > 2)
            || (ver.major == 6 && ver.minor == 2 && ver.patch >= 5)
        {
            assert_eq!(res.unwrap(), String::from("test_helper.name"));
        } else {
            assert!(res
                .err()
                .unwrap()
                .to_string()
                .contains("RedisModule_GetCurrentCommandName is not available"));
        }
    }

    Ok(())
}

#[test]
fn test_helper_info() -> Result<()> {
    const MODULES: [(&str, bool); 4] = [
        ("test_helper", false),
        ("info_handler_macro", false),
        ("info_handler_builder", true),
        ("info_handler_struct", true),
    ];

    MODULES
        .into_iter()
        .try_for_each(|(module, has_dictionary)| {
            let port: u16 = 6483;
            let _guards = vec![start_valkey_server_with_module(module, port)
                .with_context(|| FAILED_TO_START_SERVER)?];
            let mut con =
                get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

            let res: String = redis::cmd("INFO")
                .arg(module)
                .query(&mut con)
                .with_context(|| format!("failed to run INFO {module}"))?;

            assert!(res.contains(&format!("{module}_field:value")));
            if has_dictionary {
                assert!(res.contains("dictionary:key=value"));
            }

            Ok(())
        })
}

#[test]
fn test_info_handler_multiple_sections() -> Result<()> {
    const MODULES: [&str; 1] = ["info_handler_multiple_sections"];

    MODULES.into_iter().try_for_each(|module| {
        let port: u16 = 6500;
        let _guards = vec![start_valkey_server_with_module(module, port)
            .with_context(|| FAILED_TO_START_SERVER)?];
        let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

        let res: String = redis::cmd("INFO")
            .arg(format!("{module}_InfoSection2"))
            .query(&mut con)
            .with_context(|| format!("failed to run INFO {module}"))?;

        assert!(res.contains(&format!("{module}_field_2:value2")));
        assert!(!res.contains(&format!("{module}_field_1:value1")));

        Ok(())
    })
}

#[allow(unused_must_use)]
#[test]
fn test_test_helper_err() -> Result<()> {
    let port: u16 = 6484;
    let _guards =
        vec![start_valkey_server_with_module("hello", port)
            .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    // Make sure embedded nulls do not cause a crash
    redis::cmd("test_helper.err")
        .arg(&["\x00\x00"])
        .query::<()>(&mut con);

    redis::cmd("test_helper.err")
        .arg(&["no crash\x00"])
        .query::<()>(&mut con);

    Ok(())
}

#[test]
fn test_string() -> Result<()> {
    let port: u16 = 6485;
    let _guards =
        vec![start_valkey_server_with_module("string", port)
            .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    redis::cmd("string.set")
        .arg(&["key", "value"])
        .exec(&mut con)
        .with_context(|| "failed to run string.set")?;

    let res: String = redis::cmd("string.get").arg(&["key"]).query(&mut con)?;

    assert_eq!(&res, "value");

    Ok(())
}

#[test]
fn test_scan() -> Result<()> {
    let port: u16 = 6486;
    let _guards = vec![start_valkey_server_with_module("scan_keys", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    redis::cmd("set")
        .arg(&["x", "1"])
        .exec(&mut con)
        .with_context(|| "failed to run string.set")?;

    redis::cmd("set")
        .arg(&["y", "1"])
        .exec(&mut con)
        .with_context(|| "failed to run string.set")?;

    let mut res: Vec<String> = redis::cmd("scan_keys").query(&mut con)?;
    res.sort();

    assert_eq!(&res, &["x", "y"]);

    Ok(())
}

#[test]
fn test_stream_reader() -> Result<()> {
    let port: u16 = 6487;
    let _guards =
        vec![start_valkey_server_with_module("stream", port)
            .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    let _: String = redis::cmd("XADD")
        .arg(&["s", "1-1", "foo", "bar"])
        .query(&mut con)
        .with_context(|| "failed to add data to the stream")?;

    let _: String = redis::cmd("XADD")
        .arg(&["s", "1-2", "foo", "bar"])
        .query(&mut con)
        .with_context(|| "failed to add data to the stream")?;

    let res: String = redis::cmd("STREAM_POP")
        .arg(&["s"])
        .query(&mut con)
        .with_context(|| "failed to run keys_pos")?;
    assert_eq!(res, "1-1");

    let res: String = redis::cmd("STREAM_POP")
        .arg(&["s"])
        .query(&mut con)
        .with_context(|| "failed to run keys_pos")?;
    assert_eq!(res, "1-2");

    let res: usize = redis::cmd("XLEN")
        .arg(&["s"])
        .query(&mut con)
        .with_context(|| "failed to add data to the stream")?;

    assert_eq!(res, 0);

    Ok(())
}

#[test]
fn test_call() -> Result<()> {
    let port: u16 = 6488;
    let _guards =
        vec![start_valkey_server_with_module("call", port)
            .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    let res: String = redis::cmd("call.test")
        .query(&mut con)
        .with_context(|| "failed to run string.set")?;

    assert_eq!(&res, "pass");

    Ok(())
}

#[test]
fn test_ctx_flags() -> Result<()> {
    let port: u16 = 6489;
    let _guards = vec![start_valkey_server_with_module("ctx_flags", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    let res: String = redis::cmd("my_role").query(&mut con)?;

    assert_eq!(&res, "master");

    Ok(())
}

#[test]
fn test_get_current_user() -> Result<()> {
    let port: u16 = 6490;
    let _guards =
        vec![start_valkey_server_with_module("acl", port).with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    let res: String = redis::cmd("get_current_user").query(&mut con)?;

    assert_eq!(&res, "default");

    Ok(())
}

#[test]
fn test_verify_acl_on_user() -> Result<()> {
    let port: u16 = 6491;
    let _guards =
        vec![start_valkey_server_with_module("acl", port).with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    let res: String = redis::cmd("verify_key_access_for_user")
        .arg(&["default", "x"])
        .query(&mut con)?;

    assert_eq!(&res, "OK");

    let res: String = redis::cmd("ACL")
        .arg(&["SETUSER", "alice", "on", ">pass", "~cached:*", "+get"])
        .query(&mut con)?;

    assert_eq!(&res, "OK");

    let res: String = redis::cmd("verify_key_access_for_user")
        .arg(&["alice", "cached:1"])
        .query(&mut con)?;

    assert_eq!(&res, "OK");

    let res: RedisResult<String> = redis::cmd("verify_key_access_for_user")
        .arg(&["alice", "not_allow"])
        .query(&mut con);

    assert!(res.is_err());
    if let Err(res) = res {
        assert_eq!(
            res.to_string(),
            "Err: User does not have permissions on key"
        );
    }

    Ok(())
}

#[test]
fn test_key_space_notifications() -> Result<()> {
    let port: u16 = 6492;
    let _guards =
        vec![start_valkey_server_with_module("events", port)
            .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    let res: usize = redis::cmd("events.num_key_miss").query(&mut con)?;
    assert_eq!(res, 0);

    let _ = redis::cmd("GET").arg(&["x"]).exec(&mut con)?;

    let res: usize = redis::cmd("events.num_key_miss").query(&mut con)?;
    assert_eq!(res, 1);

    let _: String = redis::cmd("SET").arg(&["x", "1"]).query(&mut con)?;

    let res: String = redis::cmd("GET").arg(&["num_sets"]).query(&mut con)?;
    assert_eq!(res, "1");

    Ok(())
}

#[test]
fn test_context_mutex() -> Result<()> {
    let port: u16 = 6493;
    let _guards =
        vec![start_valkey_server_with_module("threads", port)
            .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    let res: String = redis::cmd("set_static_data")
        .arg(&["foo"])
        .query(&mut con)?;
    assert_eq!(&res, "OK");

    let res: String = redis::cmd("get_static_data").query(&mut con)?;
    assert_eq!(&res, "foo");

    let res: String = redis::cmd("get_static_data_on_thread").query(&mut con)?;
    assert_eq!(&res, "foo");

    Ok(())
}

#[test]
fn test_server_event() -> Result<()> {
    let port: u16 = 6494;
    let _guards = vec![start_valkey_server_with_module("server_events", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    redis::cmd("flushall")
        .exec(&mut con)
        .with_context(|| "failed to run flushall")?;

    let res: i64 = redis::cmd("num_flushed").query(&mut con)?;

    assert_eq!(res, 1);

    redis::cmd("flushall")
        .exec(&mut con)
        .with_context(|| "failed to run flushall")?;

    let res: i64 = redis::cmd("num_flushed").query(&mut con)?;

    assert_eq!(res, 2);

    redis::cmd("config")
        .arg(&["set", "maxmemory", "1"])
        .exec(&mut con)
        .with_context(|| "failed to run config set maxmemory")?;

    let res: i64 = redis::cmd("num_max_memory_changes").query(&mut con)?;

    assert_eq!(res, 1);

    redis::cmd("config")
        .arg(&["set", "maxmemory", "0"])
        .exec(&mut con)
        .with_context(|| "failed to run config set maxmemory")?;

    let res: i64 = redis::cmd("num_max_memory_changes").query(&mut con)?;

    assert_eq!(res, 2);

    let res: i64 = redis::cmd("num_crons").query(&mut con)?;

    assert!(res > 0);

    redis::cmd("set")
        .arg(&["key", "value"])
        .exec(&mut con)
        .with_context(|| "failed to do set")?;

    //overwrite the key for KeyChangeSubevent::Overwritten to fire
    redis::cmd("set")
        .arg(&["key", "new_value"])
        .exec(&mut con)
        .with_context(|| "failed to do set")?;

    //delete key for KeyChangeSubevent::Deleted to fire
    redis::cmd("del")
        .arg(&["key"])
        .exec(&mut con)
        .with_context(|| "failed to do del")?;

    let res: i64 = redis::cmd("num_key_events").query(&mut con)?;

    //one for overwrite and one for delete
    assert_eq!(res, 2);

    // Trigger RDB save (BGSAVE command triggers persistence events)
    redis::cmd("bgsave")
        .exec(&mut con)
        .with_context(|| "failed to run bgsave")?;

    // Wait a moment for background save to start and potentially complete
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Check that persistence events were fired
    let persistence_events_after_rdb: i64 = redis::cmd("num_persistence_events").query(&mut con)?;

    //initially there are 2 persistence events (one for RDB save start and one for RDB save end)
    // after the BGSAVE command, we expect 2 more events (one for RDB save start and one for RDB save end)
    assert_eq!(persistence_events_after_rdb, 4);

    Ok(())
}

#[test]
fn test_server_event_shutdown() -> Result<()> {
    let port: u16 = 6512;
    let _guards = vec![start_valkey_server_with_module("server_events", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    // Create a txt file called shutdown_log.txt
    let shutdown_log_path = "shutdown_log.txt";
    // If it already exists, delete it
    if std::path::Path::new(shutdown_log_path).exists() {
        std::fs::remove_file(shutdown_log_path)
            .with_context(|| "failed to remove shutdown log file")?;
    }

    // Issue the SHUTDOWN command and ignore any errors
    let _ = redis::cmd("SHUTDOWN")
        .arg("NOSAVE")
        .query::<String>(&mut con);

    // Wait briefly to ensure the shutdown event is processed
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Check if the file exists and contains the string "Server shutdown callback event ..."
    let contents = std::fs::read_to_string(shutdown_log_path)
        .with_context(|| "failed to read shutdown log file")?;

    assert!(contents.contains("Server shutdown callback event ..."));

    // Delete the file
    std::fs::remove_file(shutdown_log_path)
        .with_context(|| "failed to remove shutdown log file")?;

    Ok(())
}

#[test]
fn test_client_change_event() -> Result<()> {
    let port: u16 = 6511;
    let _guards = vec![start_valkey_server_with_module("server_events", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;
    let con2: redis::Connection =
        get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    let conn_res: i64 = redis::cmd("num_connects").query(&mut con)?;
    println!("Connection result: {}", conn_res);
    assert_eq!(conn_res, 2);

    drop(con2);

    let disconn_res: i64 = redis::cmd("num_connects").query(&mut con)?;
    println!("Disconnection result: {}", disconn_res);
    assert_eq!(disconn_res, 1);

    Ok(())
}

#[test]
fn test_configuration() -> Result<()> {
    let port: u16 = 6495;
    let _guards = vec![start_valkey_server_with_module("configuration", port)
        .with_context(|| FAILED_TO_START_SERVER)?];

    let config_get = |config: &str| -> Result<String> {
        let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;
        let res: Vec<String> = redis::cmd("config")
            .arg(&["get", config])
            .query(&mut con)
            .with_context(|| "failed to run config get")?;
        Ok(res[1].clone())
    };

    let config_set = |config: &str, val: &str| -> Result<()> {
        let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;
        let res: String = redis::cmd("config")
            .arg(&["set", config, val])
            .query(&mut con)
            .map_err(|e| anyhow::anyhow!("Failed to run config set: {}", e))?;
        assert_eq!(res, "OK");
        Ok(())
    };

    assert_eq!(config_get("configuration.i64")?, "10");
    config_set("configuration.i64", "100")?;
    assert_eq!(config_get("configuration.i64")?, "100");

    assert_eq!(config_get("configuration.atomic_i64")?, "10");
    config_set("configuration.atomic_i64", "100")?;
    assert_eq!(config_get("configuration.atomic_i64")?, "100");

    assert_eq!(config_get("configuration.valkey_string")?, "default");
    config_set("configuration.valkey_string", "new")?;
    assert_eq!(config_get("configuration.valkey_string")?, "new");

    assert_eq!(config_get("configuration.string")?, "default");
    config_set("configuration.string", "new")?;
    assert_eq!(config_get("configuration.string")?, "new");

    assert_eq!(config_get("configuration.mutex_string")?, "default");
    config_set("configuration.mutex_string", "new")?;
    assert_eq!(config_get("configuration.mutex_string")?, "new");

    assert_eq!(config_get("configuration.atomic_bool")?, "yes");
    config_set("configuration.atomic_bool", "no")?;
    assert_eq!(config_get("configuration.atomic_bool")?, "no");

    assert_eq!(config_get("configuration.bool")?, "yes");
    config_set("configuration.bool", "no")?;
    assert_eq!(config_get("configuration.bool")?, "no");

    assert_eq!(config_get("configuration.enum")?, "Val1");
    config_set("configuration.enum", "Val2")?;
    assert_eq!(config_get("configuration.enum")?, "Val2");

    assert_eq!(config_get("configuration.enum_mutex")?, "Val1");
    config_set("configuration.enum_mutex", "Val2")?;
    assert_eq!(config_get("configuration.enum_mutex")?, "Val2");

    // Validate that configs can be rejected
    let value = config_set("configuration.reject_valkey_string", "rejectvalue");
    assert!(value
        .unwrap_err()
        .to_string()
        .contains("Rejected from custom string validation"));
    let value = config_set("configuration.reject_i64", "123");
    assert!(value
        .unwrap_err()
        .to_string()
        .contains("Rejected from custom i64 validation"));
    let value = config_set("configuration.reject_bool", "no");
    assert!(value
        .unwrap_err()
        .to_string()
        .contains("Rejected from custom bool validation"));
    let value = config_set("configuration.reject_enum", "Val2");
    assert!(value
        .unwrap_err()
        .to_string()
        .contains("Rejected from custom enum validation"));

    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;
    let res: i64 = redis::cmd("configuration.num_changes")
        .query(&mut con)
        .with_context(|| "failed to run configuration.num_changes")?;
    assert_eq!(res, 26); // the first configuration initialisation is counted as well, so we will get 22 changes.

    // Validate that configs with logic to reject values can also succeed
    assert_eq!(config_get("configuration.reject_valkey_string")?, "default");
    config_set("configuration.reject_valkey_string", "validvalue")?;
    assert_eq!(
        config_get("configuration.reject_valkey_string")?,
        "validvalue"
    );
    assert_eq!(config_get("configuration.reject_i64")?, "10");
    config_set("configuration.reject_i64", "11")?;
    assert_eq!(config_get("configuration.reject_i64")?, "11");
    assert_eq!(config_get("configuration.reject_bool")?, "yes");
    config_set("configuration.reject_bool", "yes")?;
    assert_eq!(config_get("configuration.reject_bool")?, "yes");
    assert_eq!(config_get("configuration.reject_enum")?, "Val1");
    config_set("configuration.reject_enum", "Val1")?;
    assert_eq!(config_get("configuration.reject_enum")?, "Val1");
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;
    let res: i64 = redis::cmd("configuration.num_changes")
        .query(&mut con)
        .with_context(|| "failed to run configuration.num_changes")?;
    assert_eq!(res, 28);

    Ok(())
}

#[test]
fn test_response() -> Result<()> {
    let port: u16 = 6496;
    let _guards = vec![start_valkey_server_with_module("response", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    redis::cmd("hset")
        .arg(&["k", "a", "b", "c", "d", "e", "b", "f", "g"])
        .exec(&mut con)
        .with_context(|| "failed to run hset")?;

    let mut res: Vec<String> = redis::cmd("map.mget")
        .arg(&["k", "a", "c", "e"])
        .query(&mut con)
        .with_context(|| "failed to run map.mget")?;

    res.sort();
    assert_eq!(&res, &["a", "b", "b", "c", "d", "e"]);

    let mut res: Vec<String> = redis::cmd("map.unique")
        .arg(&["k", "a", "c", "e"])
        .query(&mut con)
        .with_context(|| "failed to run map.unique")?;

    res.sort();
    assert_eq!(&res, &["b", "d"]);

    Ok(())
}

#[test]
fn test_command_proc_macro() -> Result<()> {
    let port: u16 = 6497;
    let _guards = vec![start_valkey_server_with_module("proc_macro_commands", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    let res: Vec<String> = redis::cmd("COMMAND")
        .arg(&["GETKEYS", "classic_keys", "x", "foo", "y", "bar"])
        .query(&mut con)
        .with_context(|| "failed to run command getkeys")?;

    assert_eq!(&res, &["x", "y"]);

    let res: Vec<String> = redis::cmd("COMMAND")
        .arg(&["GETKEYS", "keyword_keys", "foo", "x", "1", "y", "2"])
        .query(&mut con)
        .with_context(|| "failed to run command getkeys")?;

    assert_eq!(&res, &["x", "y"]);

    let res: Vec<String> = redis::cmd("COMMAND")
        .arg(&["GETKEYS", "num_keys", "3", "x", "y", "z", "foo", "bar"])
        .query(&mut con)
        .with_context(|| "failed to run command getkeys")?;

    assert_eq!(&res, &["x", "y", "z"]);

    let res: Vec<String> = redis::cmd("COMMAND")
        .arg(&["GETKEYS", "num_keys", "0", "foo", "bar"])
        .query(&mut con)
        .with_context(|| "failed to run command getkeys")?;

    assert!(res.is_empty());

    Ok(())
}

#[test]
fn test_valkey_value_derive() -> Result<()> {
    let port: u16 = 6498;
    let _guards = vec![start_valkey_server_with_module("proc_macro_commands", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    let res: Value = redis::cmd("valkey_value_derive")
        .query(&mut con)
        .with_context(|| "failed to run valkey_value_derive")?;

    assert_eq!(res.as_sequence().unwrap().len(), 22);

    let res: String = redis::cmd("valkey_value_derive")
        .arg(&["test"])
        .query(&mut con)
        .with_context(|| "failed to run valkey_value_derive")?;

    assert_eq!(res, "OK");

    Ok(())
}

#[test]
fn test_call_blocking() -> Result<()> {
    let port: u16 = 6499;
    let _guards =
        vec![start_valkey_server_with_module("call", port)
            .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    let res: Option<String> = redis::cmd("call.blocking")
        .query(&mut con)
        .with_context(|| "failed to run call.blocking")?;

    assert_eq!(res, None);

    let res: Option<String> = redis::cmd("call.blocking_from_detached_ctx")
        .query(&mut con)
        .with_context(|| "failed to run call.blocking_from_detached_ctx")?;

    assert_eq!(res, None);

    Ok(())
}

#[test]
fn test_open_key_with_flags() -> Result<()> {
    let port: u16 = 6501;
    let _guards = vec![start_valkey_server_with_module("open_key_with_flags", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    // Avoid active expriation
    redis::cmd("DEBUG")
        .arg(&["SET-ACTIVE-EXPIRE", "0"])
        .exec(&mut con)
        .with_context(|| "failed to run DEBUG SET-ACTIVE-EXPIRE")?;

    for cmd in ["open_key_with_flags.write", "open_key_with_flags.read"].into_iter() {
        redis::cmd("set")
            .arg(&["x", "1"])
            .exec(&mut con)
            .with_context(|| "failed to run set")?;

        // Set experition time to 1 second.
        redis::cmd("pexpire")
            .arg(&["x", "1"])
            .exec(&mut con)
            .with_context(|| "failed to run pexpire")?;

        // Sleep for 2 seconds, ensure expiration time has passed.
        thread::sleep(Duration::from_millis(500));

        // Open key as read only or ReadWrite with NOEFFECTS flag.
        let res = redis::cmd(cmd).arg(&["x"]).query(&mut con);
        assert_eq!(res, Ok(()));

        // Get the number of expired keys.
        let stats: String = redis::cmd("info").arg(&["stats"]).query(&mut con)?;

        // Find the number of expired keys, x,  according to the substring "expired_keys:{x}"
        let expired_keys = stats
            .match_indices("expired_keys:")
            .next()
            .map(|(i, _)| &stats[i..i + "expired_keys:".len() + 1])
            .and_then(|s| s.split(':').nth(1))
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(-1);

        // Ensure that no keys were expired.
        assert_eq!(expired_keys, 0);

        // Delete key and reset stats
        redis::cmd("del").arg(&["x"]).exec(&mut con)?;
        redis::cmd("config").arg(&["RESETSTAT"]).exec(&mut con)?;
    }

    Ok(())
}

#[test]
fn test_expire() -> Result<()> {
    let port: u16 = 6502;
    let _guards =
        vec![start_valkey_server_with_module("expire", port)
            .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    // Create a key without TTL
    redis::cmd("set")
        .arg(&["key", "value"])
        .exec(&mut con)
        .with_context(|| "failed to run set")?;

    let ttl: i64 = redis::cmd("ttl").arg(&["key"]).query(&mut con)?;
    assert_eq!(ttl, -1);

    // Set TTL on the key
    redis::cmd("expire.cmd")
        .arg(&["key", "100"])
        .exec(&mut con)
        .with_context(|| "failed to run expire.cmd")?;

    let ttl: i64 = redis::cmd("ttl").arg(&["key"]).query(&mut con)?;
    assert!(ttl > 0);

    // Remove TTL on the key
    redis::cmd("expire.cmd")
        .arg(&["key", "-1"])
        .exec(&mut con)
        .with_context(|| "failed to run expire.cmd")?;

    let ttl: i64 = redis::cmd("ttl").arg(&["key"]).query(&mut con)?;
    assert_eq!(ttl, -1);

    Ok(())
}

#[test]
fn test_alloc() -> Result<()> {
    let port: u16 = 6509;
    let _guards = vec![start_valkey_server_with_module("data_type", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    // Test set to verify allocation
    let res: i64 = redis::cmd("alloc.set")
        .arg(&["test_key", "10"])
        .query(&mut con)
        .with_context(|| "failed to run alloc.set")?;
    assert_eq!(res, 10);

    // Get value and verify content
    let res: String = redis::cmd("alloc.get")
        .arg(&["test_key"])
        .query(&mut con)
        .with_context(|| "failed to run alloc.get")?;
    assert_eq!(res, "A".repeat(10));

    // Test set reallocation
    let res: i64 = redis::cmd("alloc.set")
        .arg(&["test_key", "5"])
        .query(&mut con)
        .with_context(|| "failed to run alloc.set")?;
    assert_eq!(res, 5);

    // Test get with reallocated key
    let res: String = redis::cmd("alloc.get")
        .arg(&["test_key"])
        .query(&mut con)
        .with_context(|| "failed to run alloc.get")?;
    assert_eq!(res, "B".repeat(5));

    let _: i64 = redis::cmd("DEL")
        .arg(&["test_key"])
        .query(&mut con)
        .with_context(|| "failed to run DEL")?;

    // Test get with deleted key
    let res: Option<String> = redis::cmd("alloc.get")
        .arg(&["test_key"])
        .query(&mut con)
        .with_context(|| "failed to run alloc.get")?;
    assert!(res.is_none());

    Ok(())
}

#[test]
fn test_debug() -> Result<()> {
    let port: u16 = 6504;
    let _guards = vec![start_valkey_server_with_module("data_type", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    let _: i64 = redis::cmd("alloc.set")
        .arg(&["test_key", "10"])
        .query(&mut con)
        .with_context(|| "failed to run alloc.set")?;

    // Test DEBUG DIGEST command to verify digest callback
    let res: String = redis::cmd("DEBUG")
        .arg("digest")
        .query(&mut con)
        .with_context(|| "failed to run DEBUG DIGEST")?;
    assert!(
        !res.is_empty(),
        "DEBUG DIGEST should return a non-empty string"
    );

    // Test DEBUG DIGEST-VALUE command to verify digest callback
    let res: redis::Value = redis::cmd("DEBUG")
        .arg(&["digest-value", "test_key"])
        .query(&mut con)
        .with_context(|| "failed to run DEBUG DIGEST-VALUE")?;
    assert!(
        !matches!(res, redis::Value::Nil),
        "DEBUG DIGEST-VALUE should not return nil"
    );

    let _: i64 = redis::cmd("DEL")
        .arg("test_key")
        .query(&mut con)
        .with_context(|| "failed to run DEL")?;

    // Test DEBUG digest command to verify digest callback on unset key
    let res: String = redis::cmd("DEBUG")
        .arg("digest")
        .query(&mut con)
        .with_context(|| "failed to run DEBUG DIGEST")?;
    assert_eq!(res, "0".repeat(40));

    // Start testing add_long_long

    // DB1
    let port: u16 = 6505;
    let _guards = vec![start_valkey_server_with_module("data_type2", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con1 = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    // DB2
    let port2: u16 = 6506;
    let _guards = vec![start_valkey_server_with_module("data_type2", port2)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con2 = get_valkey_connection(port2).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    // Set on DB1
    let _: i64 = redis::cmd("alloc2.set")
        .arg(&["k1", "3"])
        .query(&mut con1)
        .with_context(|| "failed to run alloc2.set")?;

    // Test DEBUG DIGEST command on DB1 to verify digest callback
    let res: String = redis::cmd("DEBUG")
        .arg("digest")
        .query(&mut con1)
        .with_context(|| "failed to run DEBUG DIGEST")?;
    assert!(
        !res.is_empty(),
        "DEBUG DIGEST should return a non-empty string"
    );

    // Get on DB1
    let get_res_db1: String = redis::cmd("alloc2.get")
        .arg("k1")
        .query(&mut con1)
        .with_context(|| "failed to run DEBUG DIGEST")?;
    assert!(
        !get_res_db1.is_empty(),
        "alloc.get should return a non-empty string"
    );

    // Set on DB2
    let _: i64 = redis::cmd("alloc2.set")
        .arg(&["k1", "3"])
        .query(&mut con2)
        .with_context(|| "failed to run alloc2.set")?;

    // Test DEBUG DIGEST command on DB2 to verify digest callback
    let res: String = redis::cmd("DEBUG")
        .arg("digest")
        .query(&mut con2)
        .with_context(|| "failed to run DEBUG DIGEST")?;
    assert!(
        !res.is_empty(),
        "DEBUG DIGEST should return a non-empty string"
    );

    // Get on DB2
    let get_res_db2: String = redis::cmd("alloc2.get")
        .arg("k1")
        .query(&mut con2)
        .with_context(|| "failed to run DEBUG DIGEST")?;
    assert!(
        !get_res_db2.is_empty(),
        "DEBUG DIGEST should return a non-empty string"
    );

    // Compare digested DB1 & DB2
    assert_eq!(get_res_db1, get_res_db2);

    // Delete key on DB1
    let _: i64 = redis::cmd("DEL")
        .arg("k1")
        .query(&mut con1)
        .with_context(|| "failed to run DEL")?;

    // Test DEBUG DIGEST on DB1 to verify digest callback on unset key
    let res_db1: String = redis::cmd("DEBUG")
        .arg("digest")
        .query(&mut con1)
        .with_context(|| "failed to run DEBUG DIGEST")?;
    assert_eq!(res_db1, "0".repeat(40));

    // Delete key on DB2
    let _: i64 = redis::cmd("DEL")
        .arg("k1")
        .query(&mut con2)
        .with_context(|| "failed to run DEL")?;

    // Test DEBUG DIGEST command on DB2 to verify digest callback on unset key
    let res_db2: String = redis::cmd("DEBUG")
        .arg("digest")
        .query(&mut con2)
        .with_context(|| "failed to run DEBUG DIGEST")?;
    assert_eq!(res_db2, "0".repeat(40));

    // Compare empty DB1 & DB2
    assert_eq!(res_db1, res_db2);

    Ok(())
}

#[test]
fn test_acl_categories() -> Result<()> {
    let port = 6503;
    let _guards =
        vec![start_valkey_server_with_module("acl", port).with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;
    // Get all commands that have the ACL read
    let response_data: Vec<String> = redis::cmd("COMMAND")
        .arg(&["LIST", "FILTERBY", "ACLCAT", "read"])
        .query(&mut con)
        .with_context(|| "failed to get list of commands associated with read")?;

    // Check if the list of returned commands contains existing_categories which is a new command we added the read ACL to
    let search_str = String::from("existing_categories");
    assert!(response_data.contains(&search_str));

    // Get all commands that have the custom ACL custom_acl_one
    let response_data: Vec<String> = redis::cmd("COMMAND")
        .arg(&["LIST", "FILTERBY", "ACLCAT", "custom_acl_one"])
        .query(&mut con)
        .with_context(|| "failed to get list of commands associated with custom_acl_one")?;
    // Check if the two commands we added this custom acl to are returned
    let search_str = String::from("custom_category");
    assert!(response_data.contains(&search_str));
    let search_str = String::from("custom_categories");
    assert!(response_data.contains(&search_str));

    // Get all commands that have the custom ACL custom_acl_two
    let response_data: Vec<String> = redis::cmd("COMMAND")
        .arg(&["LIST", "FILTERBY", "ACLCAT", "custom_acl_two"])
        .query(&mut con)
        .with_context(|| "failed to get list of commadns associated with custom_acl_two")?;
    // Check if the two commands we added this custom acl to are returned
    let search_str = String::from("custom_categories");
    assert!(response_data.contains(&search_str));
    Ok(())
}

#[test]
fn test_defrag() -> Result<()> {
    let port = 6510;
    let _guards = vec![start_valkey_server_with_module("data_type", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;
    // Defrag is only compatible with the defualt allocator and is not compatible with ASAN builds. If we see that the server is compiled
    // with not the default allocator we then exit this test early and don't test defrag
    let memory_info: String = redis::cmd("info")
        .arg("memory")
        .query(&mut con)
        .with_context(|| "Failed to run info memory")?;
    if memory_info.contains("mem_allocator:libc") {
        return Ok(());
    }
    // Set configs so active defrag will be able to run even with little defragmentation
    redis::cmd("config")
        .arg(&["set", "activedefrag", "yes"])
        .exec(&mut con)
        .with_context(|| "failed to run config set activedefrag")?;
    redis::cmd("config")
        .arg(&["set", "active-defrag-threshold-lower", "0"])
        .exec(&mut con)
        .with_context(|| "failed to run config set active-defrag-threshold-lower")?;
    redis::cmd("config")
        .arg(&["set", "active-defrag-ignore-bytes", "1"])
        .exec(&mut con)
        .with_context(|| "failed to run config set active-defrag-ignore-bytes")?;
    // Create some keys for active defrag to work on
    for i in 1..10000 {
        let key = format!("test_key_{}", i);
        let _: i64 = redis::cmd("alloc.set")
            .arg(&[&key, "500"])
            .query(&mut con)
            .with_context(|| "failed to run alloc.set")?;
    }
    let info: String = redis::cmd("info")
        .arg("stats")
        .query(&mut con)
        .with_context(|| "failed to run info stats")?;
    assert!(!(info.contains("active_defrag_misses:0") || !(info.contains("active_defrag_hits:0"))));
    assert!(!(info.contains("total_active_defrag_time:0")));
    // Check that the getting the values that have been defragged doesn't crash and that the return value is what we expect
    for i in 1..1000 {
        let key = format!("test_key_{}", i);
        let get_return: String = redis::cmd("alloc.get")
            .arg(key)
            .query(&mut con)
            .with_context(|| "failed to run alloc.set")?;
        assert!(get_return == "A".repeat(500));
    }
    Ok(())
}

#[test]
fn test_client() -> Result<()> {
    let port = 6507;
    let _guards =
        vec![start_valkey_server_with_module("client", port)
            .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;
    // Test client.id command
    redis::cmd("client.id")
        .exec(&mut con)
        .with_context(|| "failed execute client.id")?;
    // Test client.set_name
    redis::cmd("client.set_name")
        .arg("test_client")
        .exec(&mut con)
        .with_context(|| "failed execute client.set_name")?;
    // test client.get_name
    let resp: String = redis::cmd("client.get_name")
        .query(&mut con)
        .with_context(|| "failed execute client.get_name")?;
    assert_eq!(resp, "test_client");
    // test client.username
    let resp: String = redis::cmd("client.username")
        .query(&mut con)
        .with_context(|| "failed execute client.username")?;
    assert_eq!(resp, "default");
    // test client.cert
    let resp: String = redis::cmd("client.cert")
        .query(&mut con)
        .with_context(|| "failed execute client.cert")?;
    assert_eq!(resp, "");
    // test client.info
    let resp: String = redis::cmd("client.info")
        .query(&mut con)
        .with_context(|| "failed execute client.info")?;
    assert_eq!(resp, "1");
    // Test client.ip command
    let resp: String = redis::cmd("client.ip")
        .query(&mut con)
        .with_context(|| "failed execute client.ip")?;
    assert_eq!(resp, "127.0.0.1");
    // Test client.deauth
    let result = redis::cmd("client.deauth").arg(0).query::<String>(&mut con);
    match result {
        Ok(resp) => {
            assert_eq!(resp, "OK");
        }
        Err(err) => {
            let error_msg = err.to_string();
            assert!(error_msg.contains("Failed: to deauthenticate and close client"));
        }
    }
    let resp: String = redis::cmd("client.config_get")
        .arg("maxmemory-policy")
        .query(&mut con)
        .with_context(|| "failed execute client.config_get")?;
    assert_eq!(resp, "noeviction");
    Ok(())
}

#[test]
fn test_filter() -> Result<()> {
    let port = 6508;
    let _guards =
        vec![start_valkey_server_with_module("filter1", port)
            .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;
    // load filter2 module
    redis::cmd("MODULE")
        .arg(&["LOAD", get_module_path("filter2").unwrap().as_str()])
        .exec(&mut con)
        .with_context(|| "failed to filter2 module")?;

    // test filter1 module filter for info command
    let resp: String = redis::cmd("info")
        .query(&mut con)
        .with_context(|| "failed execute info")?;
    assert_eq!(resp, "info2\n");
    // test the set filter function and verify key/value were replaced
    let _: () = con.set("foo", "bar")?;
    let resp2: String = con.get("new_key")?;
    assert_eq!(resp2, "new_value");

    // unload the module
    redis::cmd("MODULE")
        .arg(&["UNLOAD", "filter1"])
        .exec(&mut con)
        .with_context(|| "failed to unload filter1 module")?;
    // verify the filters are unloaded and commands work as expected
    let resp3: String = redis::cmd("info")
        .query(&mut con)
        .with_context(|| "failed execute info")?;
    assert_ne!(resp3, "info2");
    let _: () = con.set("foo", "bar")?;
    let resp4: String = con.get("foo")?;
    assert_eq!(resp4, "bar");

    Ok(())
}

// Tests basic non-blocking authentication functionality.
// Verifies that:
// - Users can be created and authenticated successfully with correct credentials
// - Authentication fails when incorrect passwords are provided
#[test]
fn test_non_blocking_auth_callbacks() -> Result<()> {
    let port = 6513;
    let _guards =
        vec![start_valkey_server_with_module("auth", port)
            .with_context(|| FAILED_TO_START_SERVER)?];

    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    // Module users - these are handled by the module
    let module_users = [("user1", "module_pass1"), ("user2", "module_pass2")];

    // Engine users - completely different users that should fall through to engine
    let engine_users = [
        ("engine_user1", "engine_pass1"),
        ("engine_user2", "engine_pass2"),
    ];

    // Setup ACL users - both module users (no engine pass) and engine users (with pass)
    setup_acl_users(
        &mut con,
        &module_users
            .iter()
            .map(|(u, _)| (*u, None))
            .collect::<Vec<_>>(),
    )?;
    setup_acl_users(
        &mut con,
        &engine_users
            .iter()
            .map(|(u, p)| (*u, Some(*p)))
            .collect::<Vec<_>>(),
    )?;

    // Test 1: Module authentication
    for (user, module_pass) in module_users.iter() {
        // Correct module password should succeed
        check_auth(&mut con, user, module_pass, AuthExpectedResult::Success)?;
        // Wrong password for module user should be denied by module
        check_auth(&mut con, user, "wrong", AuthExpectedResult::Denied)?;
    }

    // Test 2: Engine authentication (different users falling through)
    for (user, engine_pass) in engine_users.iter() {
        // Engine users should succeed with correct password
        check_auth(&mut con, user, engine_pass, AuthExpectedResult::Success)?;
        // Engine users should fail with wrong password
        check_auth(&mut con, user, "wrong", AuthExpectedResult::EngineDenied)?;
    }

    Ok(())
}

// Tests blocking authentication functionality.
// Verifies that:
// - Authentication requests can block for specified duration
// - Blocked clients are correctly reported in server metrics
// - Successful and failed authentications work as expected
// - Authentication can be aborted
#[test]
fn test_blocking_auth_callbacks() -> Result<()> {
    let port = 6514;
    let _guards =
        vec![start_valkey_server_with_module("auth", port)
            .with_context(|| FAILED_TO_START_SERVER)?];

    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;
    let mut con2 = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;
    let mut con3 = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    // Set up users for blocking auth
    let users = [
        ("blockUser4", "module_blockPass4"),
        ("blockUser5", "module_blockPass5"),
        ("blockUser6", "module_blockPass6"),
        ("blockUserDelay", "blockPassDelay"),
    ];

    // Set up engine users
    let engine_users = [
        ("engine_user1", "engine_pass1"),
        ("engine_user2", "engine_pass2"),
    ];

    // Setup ACL users (no passwords needed)
    setup_acl_users(
        &mut con,
        &users.iter().map(|(u, _)| (*u, None)).collect::<Vec<_>>(),
    )?;
    // Setup engine users (with passwords)
    setup_acl_users(
        &mut con,
        &engine_users
            .iter()
            .map(|(u, p)| (*u, Some(*p)))
            .collect::<Vec<_>>(),
    )?;

    // Start auth on first connection with blockUserDelay user as it would create
    // 2-second delay for the testing purpose
    let auth_handle = std::thread::spawn(move || {
        let _: RedisResult<String> = redis::cmd("AUTH")
            .arg(&["blockUserDelay", "blockPassDelay"])
            .query(&mut con);
    });

    // Wait half a second (during the 2-second delay) to check blocked clients
    // as the engine would take some time to reflect in its metrics
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Check blocked_clients from second connection
    let blocked_clients = check_blocked_clients(&mut con2)?;
    assert!(
        blocked_clients > 0,
        "Expected to see blocked clients during auth"
    );

    // Wait for auth to complete
    auth_handle.join().unwrap();

    // Test successful blocking authentications with third connection
    for (user, module_pass) in users.iter() {
        check_auth(&mut con3, user, module_pass, AuthExpectedResult::Success)?;
    }

    // Test failed blocking authentications
    for (user, _) in users.iter() {
        check_auth(&mut con3, user, "wrong", AuthExpectedResult::Denied)?;
    }

    // Test engine authentication (fallback)
    for (user, pass) in engine_users.iter() {
        check_auth(&mut con3, user, pass, AuthExpectedResult::Success)?;
        check_auth(&mut con3, user, "wrong", AuthExpectedResult::EngineDenied)?;
    }

    // Test abort authentication case
    check_auth(
        &mut con3,
        "blockAbort",
        "abort",
        AuthExpectedResult::Aborted,
    )?;

    Ok(())
}

// This test verifies that multiple concurrent authentication callbacks are properly isolated
// and the correct callback is invoked for each client, even under concurrent load.
//
// Test scenario:
// 1. Sets up two groups of users:
//    - users_auth1: Uses blocking_auth_callback_one (fast authentication)
//    - users_auth2: Uses blocking_auth_callback_two (2-second delayed authentication for some users)
//
// 2. Creates authentication requests in this order:
//    a. Starts auth2 requests first
//    b. While auth2 requests are blocked, immediately starts auth1 requests
//
// 3. Verifies callback isolation by leveraging reply callback name in deny
// error response in this example and ensure:
//    - All auth1 users get errors containing "blocked_auth_reply_one" reply callback
//    - All auth2 users get errors containing "blocked_auth_reply_two" reply callback
//
// This test demonstrates that:
// - Authentication callbacks remain isolated even under concurrent load
// - The correct callback is invoked for each user group
// - Callbacks are not mixed up even when faster authentications (auth1)
//   complete while slower ones (auth2 with delay) are still processing
#[test]
fn test_multiple_inflight_blocking_auth_callbacks() -> Result<()> {
    let port = 6515;
    let _guards =
        vec![start_valkey_server_with_module("auth", port)
            .with_context(|| FAILED_TO_START_SERVER)?];

    let mut setup_con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    // Set up users for both auth callbacks with (username, module_pass)
    let users_auth1 = [
        ("blockUser1", "module_blockPass1"),
        ("blockUser2", "module_blockPass2"),
        ("blockUser3", "module_blockPass3"),
    ];

    let users_auth2 = [
        ("blockUser4", "module_blockPass4"),
        ("blockUser5", "module_blockPass5"),
        ("blockUser6", "module_blockPass6"),
        ("blockUserDelay", "blockPassDelay"),
    ];

    // Create all users in ACL with no passwords
    setup_acl_users(
        &mut setup_con,
        &users_auth1
            .iter()
            .map(|(u, _)| (*u, None))
            .collect::<Vec<_>>(),
    )?;
    setup_acl_users(
        &mut setup_con,
        &users_auth2
            .iter()
            .map(|(u, _)| (*u, None))
            .collect::<Vec<_>>(),
    )?;

    // Start multiple auth2 connections with wrong passwords
    let auth2_handles: Vec<_> = users_auth2
        .iter()
        .map(|(user, _)| {
            let mut con = get_valkey_connection(port)
                .with_context(|| FAILED_TO_CONNECT_TO_SERVER)
                .unwrap();
            let user = user.to_string();
            let wrong_pass = "wrong".to_string();

            std::thread::spawn(move || {
                let res: RedisResult<String> = redis::cmd("AUTH")
                    .arg(&[&user, &wrong_pass])
                    .query(&mut con);
                (user, res)
            })
        })
        .collect();

    // While auth2 requests are blocked, try auth1 requests with wrong passwords
    let auth1_handles: Vec<_> = users_auth1
        .iter()
        .map(|(user, _)| {
            let mut con = get_valkey_connection(port)
                .with_context(|| FAILED_TO_CONNECT_TO_SERVER)
                .unwrap();
            let user = user.to_string();
            let wrong_pass = "wrong".to_string();

            thread::spawn(move || {
                let res: RedisResult<String> = redis::cmd("AUTH")
                    .arg(&[&user, &wrong_pass])
                    .query(&mut con);
                (user, res)
            })
        })
        .collect();

    let auth2_results: Vec<(String, RedisResult<String>)> = auth2_handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();

    let auth1_results: Vec<(String, RedisResult<String>)> = auth1_handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();

    // Verify auth2 results have the correct callback error message
    for (user, result) in auth2_results {
        match result {
            Ok(_) => panic!("Authentication should have failed for user {}", user),
            Err(e) => {
                let err_str = e.to_string();
                assert!(
                    err_str.contains("blocked_auth_reply_two"),
                    "Wrong callback for user {}: expected 'blocked_auth_reply_two' in error, got: {}",
                    user,
                    err_str
                );
            }
        }
    }

    // Verify auth1 results have the correct callback error message
    for (user, result) in auth1_results {
        match result {
            Ok(_) => panic!("Authentication should have failed for user {}", user),
            Err(e) => {
                let err_str = e.to_string();
                assert!(
                    err_str.contains("blocked_auth_reply_one"),
                    "Wrong callback for user {}: expected 'blocked_auth_reply_one' in error, got: {}",
                    user,
                    err_str
                );
            }
        }
    }

    Ok(())
}

// This test verifies proper cleanup of blocked clients during authentication when a client disconnects.
//
//
// The test ensures:
// - Proper cleanup of blocked client resources when client disconnects mid-authentication
// - No memory leaks occur (when run with Address Sanitizer)
// - Server maintains correct blocked client count
//
// This simulates real-world scenarios where clients may disconnect during authentication,
// such as network issues or client termination.
#[test]
fn test_disconnect_client_during_blocking_auth() -> Result<()> {
    let port = 6516;
    let _guards =
        vec![start_valkey_server_with_module("auth", port)
            .with_context(|| FAILED_TO_START_SERVER)?];

    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;
    let mut monitor_con =
        get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    // Set up test user with delay for testing blocking behavior
    let users = [("blockUserDelay", "blockPassDelay")];
    setup_acl_users(
        &mut con,
        &users.iter().map(|(u, _)| (*u, None)).collect::<Vec<_>>(),
    )?;

    // Start auth in a separate thread that will be disconnected
    let users_clone = users; // Clone the array for the thread
    let auth_handle = std::thread::spawn(move || {
        let (user, module_pass) = users_clone[0]; // Using the first (and only) user
        let result: RedisResult<String> =
            redis::cmd("AUTH").arg(&[user, module_pass]).query(&mut con);

        // We expect this to fail due to connection being closed
        match result {
            Ok(_) => panic!("Authentication should have failed due to disconnection"),
            Err(e) => {
                let err = e.to_string();
                assert!(
                    err.contains("end of file"),
                    "Expected 'end of file' error, got: {}",
                    err
                );
            }
        }
    });

    // Wait to ensure auth is blocked and reflected in server metrics
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Verify client is blocked
    let blocked_clients = check_blocked_clients(&mut monitor_con)?;
    assert!(
        blocked_clients > 0,
        "Expected to see blocked clients during auth"
    );

    // Force disconnect all clients except our monitoring connection
    redis::cmd("CLIENT")
        .arg(&["KILL", "SKIPME", "YES"])
        .query::<()>(&mut monitor_con)?;

    // Verify blocked clients count is now 0
    let blocked_clients_after = check_blocked_clients(&mut monitor_con)?;
    assert_eq!(
        blocked_clients_after, 0,
        "Expected no blocked clients after disconnect"
    );

    // Wait for auth thread to complete and handle any panics
    auth_handle.join().unwrap_or_else(|e| {
        panic!("Auth thread panicked: {:?}", e);
    });

    // Verify we can still authenticate with a new connection
    let mut verify_con =
        get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;
    let (user, module_pass) = users[0];
    check_auth(
        &mut verify_con,
        user,
        module_pass,
        AuthExpectedResult::Success,
    )?;

    Ok(())
}

#[test]
fn test_preload() -> Result<()> {
    let port = 6517;
    let _guards =
        vec![start_valkey_server_with_module("preload", port)
            .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;
    // unload the module
    redis::cmd("MODULE")
        .arg(&["UNLOAD", "preload"])
        .exec(&mut con)
        .with_context(|| "failed to unload module")?;

    Ok(())
}

#[test]
fn test_subcmd() -> Result<()> {
    let port = 6518;
    let _guards =
        vec![start_valkey_server_with_module("subcmd", port)
            .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    // test cmd1
    let resp: Vec<String> = redis::cmd("cmd1")
        .query(&mut con)
        .with_context(|| "failed execute cmd1")?;
    // array([simple-string("cmd1 - top level command"), simple-string("cmd1 s1 - first level subcommand"), simple-string("cmd1 s1 s1 - second level command"), simple-string("cmd1 s1 s1 s1 - third level command"), simple-string("cmd1 help - display this message")]))
    assert_eq!(resp.len(), 5);
    // test cmd1 help
    let resp: Vec<String> = redis::cmd("cmd1")
        .arg(&["help"])
        .query(&mut con)
        .with_context(|| "failed execute cmd1 help")?;
    assert_eq!(resp.len(), 5);
    // test cmd1 info key
    let resp: Vec<String> = redis::cmd("cmd1")
        .arg(&["info", "key"])
        .query(&mut con)
        .with_context(|| "failed execute cmd1 info")?;
    assert!(resp == vec!["key", "value"]);
    // test cmd1 info array
    let resp: Vec<Vec<String>> = redis::cmd("cmd1")
        .arg(&["info", "array"])
        .query(&mut con)
        .with_context(|| "failed execute cmd1 info")?;
    assert!(resp.len() == 2);
    assert!(resp[1] == vec!["a", "b", "c"]);
    // test cmd1 s1
    let resp: String = redis::cmd("cmd1")
        .arg(&["s1"])
        .query(&mut con)
        .with_context(|| "failed execute cmd1 s1")?;
    assert_eq!(resp, "sub1");
    // test cmd1 s1 s1
    let resp: String = redis::cmd("cmd1")
        .arg(&["s1", "s1"])
        .query(&mut con)
        .with_context(|| "failed execute cmd1 s1 s1")?;
    assert_eq!(resp, "sub11");
    // test cmd1 s1 s1 s1
    let resp: String = redis::cmd("cmd1")
        .arg(&["s1", "s1", "s1"])
        .query(&mut con)
        .with_context(|| "failed execute cmd1 s1 s1 s1")?;
    assert_eq!(resp, "sub111");

    Ok(())
}

#[test]
fn test_data_type3() -> Result<()> {
    let port = 6519;
    let _guards = vec![start_valkey_server_with_module("data_type3", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    // check empty key
    redis::cmd("my-get")
        .arg(&["my-data-type1"])
        .exec(&mut con)
        .with_context(|| "failed execute my-get")?;
    // write data to different key elements
    redis::cmd("my-set-string")
        .arg(&["my-data-type1", "string1"])
        .exec(&mut con)?;
    redis::cmd("my-set-number")
        .arg(&["my-data-type1", "1"])
        .exec(&mut con)?;
    redis::cmd("my-vec-push")
        .arg(&["my-data-type1", "a"])
        .exec(&mut con)?;
    redis::cmd("my-map-insert")
        .arg(&["my-data-type1", "k1", "v1"])
        .exec(&mut con)?;
    // TODO - how to test output when it returns vector of string, number, vector and map
    let _resp: () = redis::cmd("my-get")
        .arg(&["my-data-type1"])
        .query(&mut con)
        .with_context(|| "failed execute my-get")?;
    //assert_eq!(resp, vec!["my_number", "1", "my_string", "string1"]);

    Ok(())
}

#[test]
fn test_crontab() -> Result<()> {
    let port = 6520;
    let _guards =
        vec![start_valkey_server_with_module("crontab", port)
            .with_context(|| FAILED_TO_START_SERVER)?];
    let _con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;
    // TODO - add more tests for crontab module, this just loads the module and checks that it doesn't crash
    Ok(())
}

#[test]
fn test_master_link_change_event() -> Result<()> {
    let primary_port: u16 = 6521;
    let _guards = vec![
        start_valkey_server_with_module("server_events", primary_port)
            .with_context(|| FAILED_TO_START_SERVER)?,
    ];
    let mut primary_con =
        get_valkey_connection(primary_port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    let replica_port: u16 = 6522;
    let _guards = vec![
        start_valkey_server_with_module("server_events", replica_port)
            .with_context(|| FAILED_TO_START_SERVER)?,
    ];
    let mut replica_con =
        get_valkey_connection(replica_port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    // set replicaof to primary_port
    println!("Setting up replication");
    let _: () = redis::cmd("replicaof")
        .arg("127.0.0.1")
        .arg(primary_port)
        .query(&mut replica_con)?;

    thread::sleep(Duration::from_secs(10));

    let event_count1: i64 = redis::cmd("num_master_link_change_events").query(&mut replica_con)?;
    let is_master_link_up1: bool = redis::cmd("is_master_link_up").query(&mut replica_con)?;
    println!(
        "Verifying master link up event after replication setup: \
    num_master_link_change_events {}, is_master_link_up {}",
        event_count1, is_master_link_up1
    );
    assert_eq!(event_count1, 1);
    assert!(is_master_link_up1);

    // shut down server on primary port
    println!("Shutting down primary server");
    match redis::cmd("SHUTDOWN")
        .arg("NOSAVE")
        .query::<String>(&mut primary_con)
    {
        Ok(_) => {
            println!("Primary server shut down successfully")
        }
        Err(e) if e.is_io_error() => {
            println!("Primary server shut down successfully")
        }
        Err(_) => {
            println!("Failed to shut down primary server")
        }
    }
    drop(primary_con);

    thread::sleep(Duration::from_secs(5));

    let event_count2: i64 = redis::cmd("num_master_link_change_events").query(&mut replica_con)?;
    let is_master_link_up2: bool = redis::cmd("is_master_link_up").query(&mut replica_con)?;
    println!(
        "Verifying master link up event after primary shutdown: \
    num_master_link_change_events {}, is_master_link_up {}",
        event_count2, is_master_link_up2
    );
    assert_eq!(event_count2, 2);
    assert!(!is_master_link_up2);

    Ok(())
}

#[test]
fn test_fork_child_event() -> Result<()> {
    let port: u16 = 6523;
    let _guards = vec![start_valkey_server_with_module("server_events", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    redis::cmd("bgsave")
        .exec(&mut con)
        .with_context(|| "failed to run bgsave")?;

    let num_fork_child_event1: i64 = redis::cmd("num_fork_child_events").query(&mut con)?;
    assert_eq!(num_fork_child_event1, 1);

    // Wait a moment for background save to start and potentially complete
    thread::sleep(Duration::from_millis(100));

    let num_fork_child_event2: i64 = redis::cmd("num_fork_child_events").query(&mut con)?;
    assert_eq!(num_fork_child_event2, 2);

    Ok(())
}

#[test]
fn test_replica_change_event() -> Result<()> {
    let primary_port: u16 = 6524;
    let _guards = vec![
        start_valkey_server_with_module("server_events", primary_port)
            .with_context(|| FAILED_TO_START_SERVER)?,
    ];
    let mut primary_con =
        get_valkey_connection(primary_port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    let replica_port: u16 = 6525;
    let _guards = vec![
        start_valkey_server_with_module("server_events", replica_port)
            .with_context(|| FAILED_TO_START_SERVER)?,
    ];
    let mut replica_con =
        get_valkey_connection(replica_port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    // setup replication
    let _: () = redis::cmd("replicaof")
        .arg("127.0.0.1")
        .arg(primary_port)
        .query(&mut replica_con)?;
    // need to wait for replication to establish and event to fire
    thread::sleep(Duration::from_millis(5000));
    // check replica change event on the primary
    let event_count1: i64 = redis::cmd("num_replica_change_events").query(&mut primary_con)?;
    assert_eq!(event_count1, 1);

    // disable replication
    let _: () = redis::cmd("replicaof")
        .arg("no")
        .arg("one")
        .query(&mut replica_con)?;
    // check replica change event on the primary
    let event_count2: i64 = redis::cmd("num_replica_change_events").query(&mut primary_con)?;
    assert_eq!(event_count2, 2);

    Ok(())
}

#[test]
fn test_repl_asnc_load_event() -> Result<()> {
    let primary_port: u16 = 6526;
    let _guards = vec![
        start_valkey_server_with_module("server_events", primary_port)
            .with_context(|| FAILED_TO_START_SERVER)?,
    ];
    let mut primary_con =
        get_valkey_connection(primary_port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;
    // repl-diskless-load swapdb
    let _: () = redis::cmd("config")
        .arg(&["set", "repl-diskless-load", "swapdb"])
        .exec(&mut primary_con)
        .with_context(|| "failed to run config set repl-diskless-load")?;

    let replica_port: u16 = 6527;
    let _guards = vec![
        start_valkey_server_with_module("server_events", replica_port)
            .with_context(|| FAILED_TO_START_SERVER)?,
    ];
    let mut replica_con =
        get_valkey_connection(replica_port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;
    // repl-diskless-load swapdb
    let _: () = redis::cmd("config")
        .arg(&["set", "repl-diskless-load", "swapdb"])
        .exec(&mut replica_con)
        .with_context(|| "failed to run config set repl-diskless-load")?;

    // setup replication
    let _: () = redis::cmd("replicaof")
        .arg("127.0.0.1")
        .arg(primary_port)
        .query(&mut replica_con)?;
    // need to wait for replication to establish and event to fire
    thread::sleep(Duration::from_millis(5000));

    // check num_repl_async_load_events on the replica
    let event_count1: i64 = redis::cmd("num_repl_async_load_events").query(&mut replica_con)?;
    // started and completed fire so result is 2, aborted event does not fire
    assert_eq!(event_count1, 2);

    Ok(())
}

#[test]
fn test_swapdb_event() -> Result<()> {
    let port: u16 = 6528;
    let _guards = vec![start_valkey_server_with_module("server_events", port)
        .with_context(|| FAILED_TO_START_SERVER)?];
    let mut con = get_valkey_connection(port).with_context(|| FAILED_TO_CONNECT_TO_SERVER)?;

    // run swapdb between db 0 and db 1
    let _: () = redis::cmd("swapdb").arg(&["0", "1"]).query(&mut con)?;
    // check swapdb event count
    let event_count1: i64 = redis::cmd("num_swapdb_events").query(&mut con)?;
    assert_eq!(event_count1, 1);

    // run swapdb between db 1 and db 2
    let _: () = redis::cmd("swapdb").arg(&["1", "2"]).query(&mut con)?;
    // check swapdb event count
    let event_count2: i64 = redis::cmd("num_swapdb_events").query(&mut con)?;
    assert_eq!(event_count2, 2);

    Ok(())
}
