extern crate bindgen;
extern crate cc;

use bindgen::callbacks::{IntKind, ParseCallbacks};
use std::env;
use std::path::PathBuf;

#[derive(Debug)]
struct ValkeyModuleCallback;

impl ParseCallbacks for ValkeyModuleCallback {
    fn int_macro(&self, name: &str, _value: i64) -> Option<IntKind> {
        if name.starts_with("REDISMODULE_SUBEVENT_")
            || name.starts_with("REDISMODULE_EVENT_")
            || name.starts_with("VALKEYMODULE_SUBEVENT_")
            || name.starts_with("VALKEYMODULE_EVENT_")
        {
            Some(IntKind::U64)
        } else if name.starts_with("REDISMODULE_REPLY_")
            || name.starts_with("REDISMODULE_KEYTYPE_")
            || name.starts_with("REDISMODULE_AUX_")
            || name == "REDISMODULE_OK"
            || name == "REDISMODULE_ERR"
            || name == "REDISMODULE_LIST_HEAD"
            || name == "REDISMODULE_LIST_TAIL"
            || name.starts_with("VALKEYMODULE_REPLY_")
            || name.starts_with("VALKEYMODULE_KEYTYPE_")
            || name.starts_with("VALKEYMODULE_AUX_")
            || name == "VALKEYMODULE_OK"
            || name == "VALKEYMODULE_ERR"
            || name == "VALKEYMODULE_LIST_HEAD"
            || name == "VALKEYMODULE_LIST_TAIL"
        {
            // These values are used as `enum` discriminants, and thus must be `isize`.
            Some(IntKind::Custom {
                name: "isize",
                is_signed: true,
            })
        } else if name.starts_with("REDISMODULE_NOTIFY_")
            || name.starts_with("VALKEYMODULE_NOTIFY_")
        {
            Some(IntKind::Int)
        } else {
            None
        }
    }
}

fn main() {
    // Share the integration engine matrix with the setup and test scripts.
    // Recompute the default test server whenever the matrix changes.
    println!("cargo:rerun-if-changed=integration-servers.conf");
    // Cargo exposes enabled features to build scripts as CARGO_FEATURE_* variables.
    // Read the API mode explicitly: a Redis compatibility level alone does not
    // enable RedisModule initialization.
    let use_redis_api = env::var_os("CARGO_FEATURE_USE_REDISMODULE_API").is_some();
    let mut selected = None;
    for line in include_str!("integration-servers.conf").lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Each row contains: directory | repository | branch | features.
        let fields: Vec<_> = line.split('|').collect();
        assert_eq!(fields.len(), 4, "invalid integration server row: {line}");
        // Redis cannot load modules initialized through ValkeyModule_Init.
        // Valkey supports both API modes, so it remains eligible in either mode.
        if fields[0].starts_with("redis-") && !use_redis_api {
            continue;
        }
        // The first feature identifies the row's compatibility level. Translate
        // its name to Cargo's environment-variable form to check whether it is enabled.
        let feature = fields[3].split(',').next().unwrap();
        let feature_env = format!("CARGO_FEATURE_{}", feature.replace('-', "_").to_uppercase());
        // Keep the first API-compatible row as a fallback. Rows are ordered oldest
        // to newest, so later feature matches replace earlier choices. With default
        // features and no Redis API mode, the fallback is currently Valkey 7.2.
        if selected.is_none() || env::var_os(feature_env).is_some() {
            selected = Some(fields[0]);
        }
    }
    // Embed the choice in the integration-test binary. INTEGRATION_TEST_SERVER
    // can override it at runtime; the test harness validates that override's API mode.
    println!(
        "cargo:rustc-env=DEFAULT_INTEGRATION_TEST_SERVER={}",
        selected
            .expect("integration server matrix has no server compatible with the enabled API mode")
    );

    // Build a Valkey pseudo-library so that we have symbols that we can link
    // against while building Rust code.
    //
    // include/redismodule.h is vendored in from the Valkey project and
    // src/redismodule.c is a stub that includes it and plays a few other
    // tricks that we need to complete the build.

    const RM_EXPERIMENTAL_API: &str = "REDISMODULE_EXPERIMENTAL_API";
    const VM_EXPERIMENTAL_API: &str = "VALKEYMODULE_EXPERIMENTAL_API";

    let mut build = cc::Build::new();

    build
        .define(RM_EXPERIMENTAL_API, None)
        .file("src/redismodule.c")
        .include("src/include/")
        .compile("redismodule");

    build
        .define(VM_EXPERIMENTAL_API, None)
        .file("src/valkeymodule.c")
        .include("src/include/")
        .compile("valkeymodule");

    let bindings_generator = bindgen::Builder::default();

    let bindings = bindings_generator
        .clang_arg(format!("-D{RM_EXPERIMENTAL_API}"))
        .clang_arg(format!("-D{VM_EXPERIMENTAL_API}"))
        .header("src/include/redismodule.h")
        .header("src/include/valkeymodule.h")
        .allowlist_var("(REDIS|Redis|VALKEY|Valkey).*")
        .blocklist_type("__darwin_.*")
        .allowlist_type("(RedisModule|ValkeyModule).*")
        .parse_callbacks(Box::new(ValkeyModuleCallback))
        .size_t_is_usize(true)
        .generate()
        .expect("error generating bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("failed to write bindings to file");
}
