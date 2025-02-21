use std::io::Cursor;
use std::os::raw::c_void;
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::defrag::Defrag;
use valkey_module::digest::Digest;
use valkey_module::native_types::ValkeyType;
use valkey_module::{raw, valkey_module, Context, NextArg, RedisModuleString, ValkeyResult, ValkeyString};

#[derive(Debug)]
struct MyType {
    data: String,
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
        defrag: Some(defrag),

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
    dig.add_string_buffer(&val.data.as_bytes());
    dig.end_sequence();
}

unsafe extern "C" fn defrag(defrag_ctx: *mut raw::RedisModuleDefragCtx, _from_key: *mut RedisModuleString, value: *mut *mut c_void,) -> i32 { 
    let defrag = Defrag::new(defrag_ctx);
    let ptr_ret = defrag.alloc(*value);
    if !ptr_ret.is_null() {
        *value= ptr_ret;
    }
    // Example usage of how shouldstopdefrag and defrag cursors would work. The data type used in this example is not complicated enough to use defrag cursors and 
    // 'should_stop_defrag' so this is just used to show how it could be used for a more compicated datatype.
    let mut cursor = defrag.get_cursor().unwrap_or(0);
    let number_of_allocations_in_our_data_type = 100;
    while cursor <  number_of_allocations_in_our_data_type && !defrag.should_stop_defrag() {
        // Perform some defrag action i.e call defrag.alloc on the inner mechanism of the data type
        cursor += 1;
    }
    // Save the cursor for where we will start defragmenting from next time
    defrag.set_cursor(cursor);
    // If not all filters were looked at, return 1 to indicate incomplete defragmentation
    if cursor < number_of_allocations_in_our_data_type {
        return 1;
    }
    // 
    0
}

fn alloc_set(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    let mut args = args.into_iter().skip(1);
    let key = args.next_arg()?;
    let size = args.next_i64()?;

    ctx.log_debug(format!("key: {key}, size: {size}").as_str());

    let key = ctx.open_key_writable(&key);

    if let Some(value) = key.get_value::<MyType>(&MY_VALKEY_TYPE)? {
        value.data = "B".repeat(size as usize);
    } else {
        let value = MyType {
            data: "A".repeat(size as usize),
        };

        key.set_value(&MY_VALKEY_TYPE, value)?;
    }
    Ok(size.into())
}

fn alloc_get(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    let mut args = args.into_iter().skip(1);
    let key = args.next_arg()?;

    let key = ctx.open_key(&key);

    let value = match key.get_value::<MyType>(&MY_VALKEY_TYPE)? {
        Some(value) => value.data.as_str().into(),
        None => ().into(),
    };

    Ok(value)
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "alloc",
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
