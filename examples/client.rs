use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{
    valkey_module, Context, NextArg, Status, ValkeyError, ValkeyResult, ValkeyString, ValkeyValue,
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
    Ok(ctx.get_client_cert()?.to_string().into())
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
    let client_id: u64 = client_id_str.parse_integer()?.try_into()?;
    let resp = ctx.deauthenticate_and_close_client_by_id(client_id);
    match resp {
        Status::Ok => Ok(ValkeyValue::from("OK")),
        Status::Err => Err(ValkeyError::Str(
            "Failed to deauthenticate and close client",
        )),
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
        ["client.config_get", config_get, "", 0, 0, 0]
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use valkey_module::RedisModuleClientInfo;

    const TEST_CLIENT_CERTIFICATE: &str = concat!(
        "-----BEGIN CERTIFICATE-----\n",
        "VGhpcyBpcyBhIHRlc3QgY2xpZW50IGNlcnRpZmljYXRlLg==\n",
        "-----END CERTIFICATE-----\n"
    );

    #[test]
    fn returns_client_id() {
        let mut context = Context::test();
        context.expect_get_client_id(42);

        let result = get_client_id(&context, Vec::new()).expect("client ID should be returned");

        assert_eq!(result, ValkeyValue::Integer(42));
    }

    #[test]
    fn returns_client_name() {
        let mut context = Context::test();
        context.expect_get_client_name_by_id(42, "alice");

        let result = get_client_name(&context, Vec::new())
            .expect("configured client name should be returned");

        assert_eq!(result, ValkeyValue::BulkString("alice".into()));
    }

    #[test]
    fn returns_client_username() {
        let mut context = Context::test();
        context.expect_get_client_username_by_id(42, "alice");

        let result = get_client_username(&context, Vec::new())
            .expect("configured client username should be returned");

        assert_eq!(result, ValkeyValue::BulkString("alice".into()));
    }

    #[test]
    fn deauthenticates_client_by_id() {
        let mut context = Context::test();
        context.expect_deauthenticate_and_close_client_by_id(42);
        let args = vec![
            context.create_string("client.deauth"),
            context.create_string("42"),
        ];

        let result = deauth_client_by_id(&context, args)
            .expect("configured client should be deauthenticated");

        assert_eq!(result, ValkeyValue::BulkString("OK".into()));
    }

    #[test]
    fn returns_client_info_version() {
        let mut context = Context::test();
        context.expect_get_client_info_by_id(RedisModuleClientInfo {
            version: 7,
            id: 42,
            ..RedisModuleClientInfo::default()
        });

        let result = get_client_info(&context, Vec::new())
            .expect("configured client info should be returned");

        assert_eq!(result, ValkeyValue::BulkString("7".into()));
    }

    #[test]
    fn returns_client_ip() {
        let mut context = Context::test();
        context.expect_get_client_ip_by_id(42, "127.0.0.1");

        let result =
            get_client_ip(&context, Vec::new()).expect("configured client IP should be returned");

        assert_eq!(result, ValkeyValue::BulkString("127.0.0.1".into()));
    }

    #[test]
    fn handles_configured_client_certificate() {
        let mut context = Context::test();
        context.expect_get_client_cert(TEST_CLIENT_CERTIFICATE);

        let result = get_client_cert(&context, Vec::new())
            .expect("configured client certificate should be handled");

        assert_eq!(
            result,
            ValkeyValue::BulkString(TEST_CLIENT_CERTIFICATE.into())
        );

        let context = Context::test();
        assert!(matches!(
            get_client_cert(&context, Vec::new()),
            Err(ValkeyError::Str("Client/Cert is null"))
        ));
    }

    #[test]
    fn sets_client_name() {
        let mut context = Context::test();
        context.expect_set_client_name_by_id(42);
        let args = vec![
            context.create_string("client.set_name"),
            context.create_string("bob"),
        ];

        let result = set_client_name(&context, args).expect("client name should be updated");

        assert_eq!(result, ValkeyValue::Integer(Status::Ok as i64));
        assert_eq!(
            context
                .get_client_name()
                .expect("updated client name should be returned")
                .as_slice(),
            b"bob"
        );
    }
}
