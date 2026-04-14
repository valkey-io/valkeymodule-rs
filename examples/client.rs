//! Client-info command handlers that accept [`ClientContextInterface`] so the
//! logic can be unit-tested with [`MockClientContext`] instead of a live
//! Valkey server. See the `tests` module below for the pattern.
//!
//! [`ClientContextInterface`]: valkey_module::ClientContextInterface
//! [`MockClientContext`]: valkey_module::MockClientContext

use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{
    valkey_module, ClientContextInterface, Context, ContextInterface, NextArg, Status, ValkeyError,
    ValkeyResult, ValkeyString, ValkeyValue,
};

fn get_client_id(ctx: &impl ClientContextInterface, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok((ctx.get_client_id() as i64).into())
}

fn get_client_name(ctx: &impl ClientContextInterface, _args: Vec<ValkeyString>) -> ValkeyResult {
    let client_name = ctx.get_client_name()?;
    Ok(ValkeyValue::from(client_name.to_string()))
}

fn get_client_username(
    ctx: &impl ClientContextInterface,
    _args: Vec<ValkeyString>,
) -> ValkeyResult {
    let client_username = ctx.get_client_username()?;
    Ok(ValkeyValue::from(client_username.to_string()))
}

fn set_client_name(ctx: &impl ClientContextInterface, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() != 2 {
        return Err(ValkeyError::WrongArity);
    }
    let mut args = args.into_iter().skip(1);
    let client_name = args.next_arg()?;
    let resp = ctx.set_client_name(&client_name);
    Ok(ValkeyValue::Integer(resp as i64))
}

fn get_client_cert(ctx: &impl ClientContextInterface, _args: Vec<ValkeyString>) -> ValkeyResult {
    // unless connection is made with cert, this will return Err
    match ctx.get_client_cert() {
        Ok(cert) => Ok(ValkeyValue::from(cert.to_string())),
        Err(_) => Ok("".into()),
    }
}

fn get_client_info(ctx: &impl ClientContextInterface, _args: Vec<ValkeyString>) -> ValkeyResult {
    let client_info = ctx.get_client_info()?;
    Ok(ValkeyValue::from(client_info.version.to_string()))
}

fn get_client_ip(ctx: &impl ClientContextInterface, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ctx.get_client_ip()?.into())
}

fn deauth_client_by_id(ctx: &impl ClientContextInterface, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() != 2 {
        return Err(ValkeyError::WrongArity);
    }
    let mut args = args.into_iter().skip(1);
    let client_id_str: ValkeyString = args.next_arg()?;
    let client_id: u64 = client_id_str.parse_integer()?.try_into()?;
    match ctx.deauthenticate_and_close_client_by_id(client_id) {
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
    let config_value = ContextInterface::config_get(ctx, &config_name.to_string())?;
    Ok(ValkeyValue::from(config_value.to_string()))
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

//////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::eq;
    use valkey_module::{MockClientContext, RedisModuleClientInfo};

    #[test]
    fn test_get_client_id() {
        let mut ctx = MockClientContext::new();
        ctx.expect_get_client_id().times(1).returning(|| 42);
        let reply = get_client_id(&ctx, vec![]).unwrap();
        assert_eq!(reply, ValkeyValue::Integer(42));
    }

    #[test]
    fn test_get_client_name() {
        let mut ctx = MockClientContext::new();
        ctx.expect_get_client_name()
            .times(1)
            .returning(|| Ok(ValkeyString::create_for_test("alice")));
        let reply = get_client_name(&ctx, vec![]).unwrap();
        assert_eq!(reply, ValkeyValue::from("alice".to_string()));
    }

    #[test]
    fn test_get_client_username() {
        let mut ctx = MockClientContext::new();
        ctx.expect_get_client_username()
            .times(1)
            .returning(|| Ok(ValkeyString::create_for_test("admin")));
        let reply = get_client_username(&ctx, vec![]).unwrap();
        assert_eq!(reply, ValkeyValue::from("admin".to_string()));
    }

    #[test]
    fn test_set_client_name() {
        let mut ctx = MockClientContext::new();
        ctx.expect_set_client_name()
            .withf(|name| name.as_slice() == b"bob")
            .times(1)
            .returning(|_| Status::Ok);
        let args = vec![
            ValkeyString::create_for_test("client.set_name"),
            ValkeyString::create_for_test("bob"),
        ];
        let reply = set_client_name(&ctx, args).unwrap();
        assert_eq!(reply, ValkeyValue::Integer(Status::Ok as i64));
    }

    #[test]
    fn test_set_client_name_wrong_arity() {
        let ctx = MockClientContext::new();
        let err = set_client_name(&ctx, vec![ValkeyString::create_for_test("client.set_name")])
            .unwrap_err();
        assert!(matches!(err, ValkeyError::WrongArity));
    }

    #[test]
    fn test_get_client_cert_err_returns_empty() {
        let mut ctx = MockClientContext::new();
        ctx.expect_get_client_cert()
            .times(1)
            .returning(|| Err(ValkeyError::Str("no cert")));
        let reply = get_client_cert(&ctx, vec![]).unwrap();
        assert_eq!(reply, ValkeyValue::from("".to_string()));
    }

    #[test]
    fn test_get_client_info() {
        let mut ctx = MockClientContext::new();
        ctx.expect_get_client_info().times(1).returning(|| {
            Ok(RedisModuleClientInfo {
                version: 1,
                flags: 0,
                id: 7,
                addr: [0; 46],
                port: 6379,
                db: 0,
            })
        });
        let reply = get_client_info(&ctx, vec![]).unwrap();
        assert_eq!(reply, ValkeyValue::from("1".to_string()));
    }

    #[test]
    fn test_get_client_ip() {
        let mut ctx = MockClientContext::new();
        ctx.expect_get_client_ip()
            .times(1)
            .returning(|| Ok("127.0.0.1".to_string()));
        let reply = get_client_ip(&ctx, vec![]).unwrap();
        assert_eq!(reply, ValkeyValue::from("127.0.0.1".to_string()));
    }

    #[test]
    #[ignore]
    fn test_deauth_client_by_id() {
        let mut ctx = MockClientContext::new();
        ctx.expect_deauthenticate_and_close_client_by_id()
            .with(eq(7u64))
            .times(1)
            .returning(|_| Status::Ok);
        let args = vec![
            ValkeyString::create_for_test("client.deauth"),
            ValkeyString::create_for_test("7"),
        ];
        let reply = deauth_client_by_id(&ctx, args).unwrap();
        assert_eq!(reply, ValkeyValue::from("OK"));
    }

    #[test]
    #[ignore]
    fn test_deauth_client_by_id_failure() {
        let mut ctx = MockClientContext::new();
        ctx.expect_deauthenticate_and_close_client_by_id()
            .with(eq(7u64))
            .times(1)
            .returning(|_| Status::Err);
        let args = vec![
            ValkeyString::create_for_test("client.deauth"),
            ValkeyString::create_for_test("7"),
        ];
        let err = deauth_client_by_id(&ctx, args).unwrap_err();
        assert!(matches!(err, ValkeyError::Str(_)));
    }
}
