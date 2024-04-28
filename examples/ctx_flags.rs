use valkey_module::{
    valkey_module, Context, ContextFlags, ValkeyResult, ValkeyString, ValkeyValue,
};

fn role(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ValkeyValue::SimpleStringStatic(
        if ctx.get_flags().contains(ContextFlags::MASTER) {
            "master"
        } else {
            "slave"
        },
    ))
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "ctx_flags",
    version: 1,
    allocator: (valkey_module::alloc::ValkeyAlloc, valkey_module::alloc::ValkeyAlloc),
    data_types: [],
    commands: [
        ["my_role", role, "readonly", 0, 0, 0],
    ],
}
