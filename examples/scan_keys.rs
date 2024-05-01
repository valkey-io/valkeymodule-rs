use valkey_module::{
    key::ValkeyKey, valkey_module, Context, KeysCursor, ValkeyResult, ValkeyString, ValkeyValue,
};

fn scan_keys(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    let cursor = KeysCursor::new();
    let mut res = Vec::new();

    let scan_callback = |_ctx: &Context, key_name: ValkeyString, _key: Option<&ValkeyKey>| {
        res.push(ValkeyValue::BulkValkeyString(key_name));
    };

    while cursor.scan(ctx, &scan_callback) {
        // do nothing
    }
    Ok(ValkeyValue::Array(res))
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "scan",
    version: 1,
    allocator: (valkey_module::alloc::ValkeyAlloc, valkey_module::alloc::ValkeyAlloc),
    data_types: [],
    commands: [
        ["scan_keys", scan_keys, "readonly", 0, 0, 0],
    ],
}
