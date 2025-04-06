use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{
    valkey_module, Context, NextArg, ValkeyError, ValkeyResult, ValkeyString, ValkeyValue,
};

fn get_client_id(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    let client_id = ctx.get_client_id();
    Ok((client_id as i64).into())
}

fn get_client_name(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    let client_name_by_id = ctx.get_client_name_by_id(ctx.get_client_id());
    ctx.log_notice(&format!(
        "client_name_by_id: {:?}",
        client_name_by_id.to_string()
    ));
    let client_name = ctx.get_client_name();
    Ok(ValkeyValue::from(client_name.to_string()))
}

fn get_client_username(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    let client_username_by_id = ctx.get_client_username_by_id(ctx.get_client_id());
    ctx.log_notice(&format!(
        "client_username_by_id: {:?}",
        client_username_by_id.to_string()
    ));
    let client_username = ctx.get_client_username();
    Ok(ValkeyValue::from(client_username.to_string()))
}

fn set_client_name(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() != 2 {
        return Err(ValkeyError::WrongArity);
    }
    let mut args = args.into_iter().skip(1);
    let client_name = args.next_arg()?;
    let resp = ctx.set_client_name(&client_name);
    Ok(ValkeyValue::Integer(resp as i64))
}

fn get_client_cert(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ValkeyValue::from(ctx.get_client_cert().to_string()))
}

fn get_client_info(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    let client_info = ctx.get_client_info();
    // return something like this:
    Ok(ValkeyValue::from(client_info.version.to_string()))
}

fn get_client_ip(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ctx.get_client_ip().into())
}

valkey_module! {
    name: "client",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["client.id", get_client_id, "", 0, 0, 0],
        ["client.get_name", get_client_name, "", 0, 0, 0],
        ["client.set_name", set_client_name, "", 0, 0, 0],
        ["client.username", get_client_username, "", 0, 0, 0],
        ["client.cert", get_client_cert, "", 0, 0, 0],
        ["client.info", get_client_info, "", 0, 0, 0],
        ["client.ip", get_client_ip, "", 0, 0, 0],
    ]
}
