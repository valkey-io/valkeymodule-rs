use valkey_module::{
    valkey_module, Context, NextArg, ValkeyError, ValkeyResult, ValkeyString, ValkeyValue,
};

fn info_cmd(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() < 3 {
        return Err(ValkeyError::WrongArity);
    }

    let mut args = args.into_iter().skip(1);

    let section = args.next_str()?;
    let field = args.next_str()?;

    let server_info = ctx.server_info(section);
    Ok(server_info
        .field(field)
        .map_or(ValkeyValue::Null, ValkeyValue::BulkValkeyString))
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "info",
    version: 1,
    allocator: (valkey_module::alloc::ValkeyAlloc, valkey_module::alloc::ValkeyAlloc),
    data_types: [],
    commands: [
        ["infoex", info_cmd, "", 0, 0, 0],
    ],
}
