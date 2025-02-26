use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{valkey_module, Context, NextArg, ValkeyError, ValkeyResult, ValkeyString};

valkey_module! {
    name: "client",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["client.id", get_client_id, "readonly", 0, 0, 0],
        ["client.name", set_client_name, "readonly", 0, 0, 0],
    ]
}

fn get_client_id(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    let client_id = ctx.get_client_id();
    ctx.log_notice(&format!(
        "client_username: {:?}",
        ctx.get_client_username().to_string()
    ));
    ctx.log_notice(&format!(
        "client_name: {:?}",
        ctx.get_client_name().to_string()
    ));
    ctx.log_notice(&format!(
        "client_cert: {:?}",
        ctx.get_client_cert().to_string()
    ));
    ctx.log_notice(&format!("client_info: {:?}", ctx.get_client_info()));
    Ok((client_id as i64).into())
}

fn set_client_name(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() != 2 {
        return Err(ValkeyError::WrongArity);
    }
    let mut args = args.into_iter().skip(1);
    let client_name = args.next_arg()?;
    let resp = ctx.set_client_name(&client_name);
    Ok(resp.into())
}
