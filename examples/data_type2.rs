use std::os::raw::c_void;
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::digest::Digest;
use valkey_module::native_types::ValkeyType;
use valkey_module::{raw, valkey_module, Context, NextArg, ValkeyResult, ValkeyString};

#[derive(Debug)]
struct MyType {
    data: i64,
}

static MY_VALKEY_TYPE: ValkeyType = ValkeyType::new(
    "mytype123",
    0,
    raw::RedisModuleTypeMethods {
        version: raw::REDISMODULE_TYPE_METHOD_VERSION as u64,
        rdb_load: None,
        rdb_save: None,
        aof_rewrite: None,
        free: Some(free),
        digest: Some(digest),
        mem_usage: None,

        // Aux data
        aux_load: None,
        aux_save: None,
        aux_save2: None,
        aux_save_triggers: 0,

        free_effort: None,
        unlink: None,
        copy: None,
        defrag: None,

        copy2: None,
        free_effort2: None,
        mem_usage2: None,
        unlink2: None,
    },
);

unsafe extern "C" fn free(value: *mut c_void) {
    drop(Box::from_raw(value.cast::<MyType>()));
}

unsafe extern "C" fn digest(md: *mut raw::RedisModuleDigest, value: *mut c_void) {
    let mut dig = Digest::new(md);
    let val = &*(value.cast::<MyType>());
    dig.add_long_long(val.data);
    dig.get_db_id();
    let keyname = dig.get_key_name();
    assert!(!keyname.is_empty());
    dig.end_sequence();
}

fn alloc_set(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    let mut args = args.into_iter().skip(1);
    let key = args.next_arg()?;
    let size = args.next_i64()?;

    ctx.log_debug(format!("key: {key}, size: {size}").as_str());

    let key = ctx.open_key_writable(&key);

    if let Some(value) = key.get_value::<MyType>(&MY_VALKEY_TYPE)? {
        value.data = size;
    } else {
        let value = MyType { data: size };
        key.set_value(&MY_VALKEY_TYPE, value)?;
    }
    Ok(size.into())
}

fn alloc_get(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    let mut args = args.into_iter().skip(1);
    let key = args.next_arg()?;

    let key = ctx.open_key(&key);

    let value = match key.get_value::<MyType>(&MY_VALKEY_TYPE)? {
        Some(value) => value.data.into(),
        None => ().into(),
    };

    Ok(value)
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "alloc2",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [
        MY_VALKEY_TYPE,
    ],
    commands: [
        ["alloc.set", alloc_set, "write", 1, 1, 1],
        ["alloc.get", alloc_get, "readonly", 1, 1, 1],
    ],
}
