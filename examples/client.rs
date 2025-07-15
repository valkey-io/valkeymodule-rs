use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{
    valkey_module, Context, NextArg, ValkeyError, ValkeyResult, ValkeyString, ValkeyValue,
};

fn get_client_id(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    let client_id = ctx.get_client_id();
    Ok((client_id as i64).into())
}

fn get_client_name(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    // test for invalid client_id
    match ctx.get_client_name_by_id(0) {
        Ok(tmp) => ctx.log_notice(&format!(
            "client_id 0 client_name_by_id: {:?}",
            tmp.to_string()
        )),
        Err(err) => ctx.log_notice(&format!("client_id 0 client_name_by_id: {:?}", err)),
    }
    let client_name = ctx.get_client_name()?;
    Ok(ValkeyValue::from(client_name.to_string()))
}

fn get_client_username(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    // test for invalid client_id
    match ctx.get_client_username_by_id(0) {
        Ok(tmp) => ctx.log_notice(&format!(
            "client_id 0 client_username_by_id: {:?}",
            tmp.to_string()
        )),
        Err(err) => ctx.log_notice(&format!("client_id 0 client_username_by_id: {:?}", err)),
    }
    let client_username = ctx.get_client_username()?;
    Ok(ValkeyValue::from(client_username.to_string()))
}

fn set_client_name(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() != 2 {
        return Err(ValkeyError::WrongArity);
    }
    let mut args = args.into_iter().skip(1);
    let client_name = args.next_arg()?;
    // test for invalid client_id
    let resp1 = ctx.set_client_name_by_id(0, &client_name);
    ctx.log_notice(&format!("client_id 0 set_client_name_by_id: {:?}", resp1));
    let resp2 = ctx.set_client_name(&client_name);
    Ok(ValkeyValue::Integer(resp2 as i64))
}

fn get_client_cert(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    // unless connection is made with cert, this will return Err, so just log it and return nothing
    match ctx.get_client_cert() {
        Ok(tmp) => ctx.log_notice(&format!("client_cert: {:?}", tmp.to_string())),
        Err(err) => ctx.log_notice(&format!("client_cert: {:?}", err.to_string())),
    }
    Ok("".into())
}

fn get_client_info(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    // test for invalid client_id
    let client_info_by_id = ctx.get_client_info_by_id(0);
    ctx.log_notice(&format!(
        "client_id 0 client_info_by_id: {:?}",
        client_info_by_id
    ));
    let client_info = ctx.get_client_info()?;
    ctx.log_notice(&format!("client_info: {:?}", client_info));
    // return version like this:
    Ok(ValkeyValue::from(client_info.version.to_string()))
}

fn get_client_ip(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    // test for invalid client_id
    let client_ip_by_id = ctx.get_client_ip_by_id(0);
    ctx.log_notice(&format!(
        "client_id 0 client_ip_by_id: {:?}",
        client_ip_by_id
    ));
    Ok(ctx.get_client_ip()?.into())
}

fn deauth_client_by_id(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() != 2 {
        return Err(ValkeyError::WrongArity);
    }
    let mut args = args.into_iter().skip(1);
    let client_id_str: ValkeyString = args.next_arg()?;
    let client_id: u64 = client_id_str.parse_integer()?.try_into().unwrap();
    let resp = ctx.deauthenticate_and_close_client_by_id(client_id);
    match resp {
        Ok(msg) => Ok(ValkeyValue::from(msg)),
        Err(err) => Err(err),
    }
}

fn config_get(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() != 2 {
        return Err(ValkeyError::WrongArity);
    }
    let mut args = args.into_iter().skip(1);
    let config_name: ValkeyString = args.next_arg()?;
    let config_value = ctx.config_get(config_name.to_string());
    match config_value {
        Ok(value) => Ok(ValkeyValue::from(value.to_string())),
        Err(err) => Err(err),
    }
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
        ["client.deauth", deauth_client_by_id, "", 0, 0, 0],
        ["client.config_get", config_get, "", 0, 0, 0],
    ]
}
