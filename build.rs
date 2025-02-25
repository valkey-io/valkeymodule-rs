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
