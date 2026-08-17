use valkey_module::{
    valkey_module, BlockedClient, CallOptionResp, CallOptionsBuilder, CallReply, CallResult,
    Context, FutureCallReply, PromiseCallReply, ThreadSafeContext, ValkeyError, ValkeyResult,
    ValkeyString, ValkeyValue,
};

use std::thread;
use valkey_module::alloc::ValkeyAlloc;

fn call_test(ctx: &Context, _: Vec<ValkeyString>) -> ValkeyResult {
    let res: String = ctx.call("ECHO", &["TEST"])?.try_into()?;
    if "TEST" != &res {
        return Err(ValkeyError::Str("Failed calling 'ECHO TEST'"));
    }

    let res: String = ctx.call("ECHO", vec!["TEST"].as_slice())?.try_into()?;
    if "TEST" != &res {
        return Err(ValkeyError::Str(
            "Failed calling 'ECHO TEST' dynamic str vec",
        ));
    }

    let res: String = ctx.call("ECHO", &[b"TEST"])?.try_into()?;
    if "TEST" != &res {
        return Err(ValkeyError::Str(
            "Failed calling 'ECHO TEST' with static [u8]",
        ));
    }

    let res: String = ctx.call("ECHO", vec![b"TEST"].as_slice())?.try_into()?;
    if "TEST" != &res {
        return Err(ValkeyError::Str(
            "Failed calling 'ECHO TEST' dynamic &[u8] vec",
        ));
    }

    let res: String = ctx.call("ECHO", &[&"TEST".to_string()])?.try_into()?;
    if "TEST" != &res {
        return Err(ValkeyError::Str("Failed calling 'ECHO TEST' with String"));
    }

    let res: String = ctx
        .call("ECHO", vec![&"TEST".to_string()].as_slice())?
        .try_into()?;
    if "TEST" != &res {
        return Err(ValkeyError::Str(
            "Failed calling 'ECHO TEST' dynamic &[u8] vec",
        ));
    }

    let res: String = ctx
        .call("ECHO", &[&ctx.create_string("TEST")])?
        .try_into()?;
    if "TEST" != &res {
        return Err(ValkeyError::Str(
            "Failed calling 'ECHO TEST' with ValkeyString",
        ));
    }

    let res: String = ctx
        .call("ECHO", vec![&ctx.create_string("TEST")].as_slice())?
        .try_into()?;
    if "TEST" != &res {
        return Err(ValkeyError::Str(
            "Failed calling 'ECHO TEST' with dynamic array of ValkeyString",
        ));
    }

    let call_options = CallOptionsBuilder::new().script_mode().errors_as_replies();
    let res: CallResult = ctx.call_ext::<&[&str; 0], _>("SHUTDOWN", &call_options.build(), &[]);
    if let Err(err) = res {
        let error_msg = err.to_utf8_string().unwrap();
        if !error_msg.contains("not allow") {
            return Err(ValkeyError::String(format!(
                "Failed to verify error messages, expected error message to contain 'not allow', error message: '{error_msg}'",
            )));
        }
    } else {
        return Err(ValkeyError::Str("Failed to set script mode on call_ext"));
    }

    // test resp3 on call_ext
    let call_options = CallOptionsBuilder::new()
        .script_mode()
        .resp(CallOptionResp::Resp3)
        .errors_as_replies()
        .build();
    ctx.call_ext::<_, CallResult>("HSET", &call_options, &["x", "foo", "bar"])
        .map_err(|e| -> ValkeyError { e.into() })?;
    let res: CallReply = ctx
        .call_ext::<_, CallResult>("HGETALL", &call_options, &["x"])
        .map_err(|e| -> ValkeyError { e.into() })?;
    if let CallReply::Map(map) = res {
        let res = map.iter().fold(Vec::new(), |mut vec, (key, val)| {
            if let CallReply::String(key) = key.unwrap() {
                vec.push(key.to_string().unwrap());
            }
            if let CallReply::String(val) = val.unwrap() {
                vec.push(val.to_string().unwrap());
            }
            vec
        });
        if res != vec!["foo".to_string(), "bar".to_string()] {
            return Err(ValkeyError::String(
                "Reply of hgetall does not match expected value".into(),
            ));
        }
    } else {
        return Err(ValkeyError::String(
            "Did not get a set type on hgetall".into(),
        ));
    }

    Ok("pass".into())
}

fn call_blocking_internal(ctx: &Context) -> PromiseCallReply<'static, '_> {
    let call_options = CallOptionsBuilder::new().build_blocking();
    ctx.call_blocking("blpop", &call_options, &["list", "1"])
}

fn call_blocking_handle_future(
    ctx: &Context,
    f: FutureCallReply<'_>,
    blocked_client: BlockedClient,
) {
    let future_handler = f.set_unblock_handler(move |_ctx, reply| {
        let thread_ctx = ThreadSafeContext::with_blocked_client(blocked_client);
        thread_ctx.reply(reply.map_or_else(|e| Err(e.into()), |v| Ok((&v).into())));
    });
    future_handler.dispose(ctx);
}

fn call_blocking(ctx: &Context, _: Vec<ValkeyString>) -> ValkeyResult {
    let res = call_blocking_internal(ctx);
    match res {
        PromiseCallReply::Resolved(r) => r.map_or_else(|e| Err(e.into()), |v| Ok((&v).into())),
        PromiseCallReply::Future(f) => {
            let blocked_client = ctx.block_client();
            call_blocking_handle_future(ctx, f, blocked_client);
            Ok(ValkeyValue::NoReply)
        }
    }
}

fn call_blocking_from_detach_ctx(ctx: &Context, _: Vec<ValkeyString>) -> ValkeyResult {
    let blocked_client = ctx.block_client();
    thread::spawn(move || {
        let ctx_guard = valkey_module::MODULE_CONTEXT.lock();
        let res = call_blocking_internal(&ctx_guard);
        match res {
            PromiseCallReply::Resolved(r) => {
                let thread_ctx = ThreadSafeContext::with_blocked_client(blocked_client);
                thread_ctx.reply(r.map_or_else(|e| Err(e.into()), |v| Ok((&v).into())));
            }
            PromiseCallReply::Future(f) => {
                call_blocking_handle_future(&ctx_guard, f, blocked_client);
            }
        }
    });
    Ok(ValkeyValue::NoReply)
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "call",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["call.test", call_test, "", 0, 0, 0],
        ["call.blocking", call_blocking, "", 0, 0, 0],
        ["call.blocking_from_detached_ctx", call_blocking_from_detach_ctx, "", 0, 0, 0],
    ],
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use valkey_module::redisvalue::ValkeyValueKey;

    #[test]
    fn context_creates_string() {
        let context = Context::test();
        let data = "test-string";

        let string = context.create_string(data);

        assert_eq!(string.as_slice(), data.as_bytes());
        assert_eq!(string.len(), data.len());
    }

    #[test]
    fn context_creates_empty_string() {
        let context = Context::test();

        let string = context.create_string("");

        assert!(string.is_empty());
    }

    #[test]
    fn context_call_returns_configured_reply() {
        let mut context = Context::test();
        context.expect_call(
            "ECHO",
            &["unit-test"],
            ValkeyValue::SimpleString("unit-test".to_owned()),
        );

        let reply: String = context
            .call("ECHO", &["unit-test"])
            .expect("configured call should return a reply")
            .try_into()
            .expect("configured string reply should convert to String");

        assert_eq!(reply, "unit-test");
    }

    #[test]
    fn call_test_accepts_configured_resp3_map_reply() {
        let mut context = Context::test();
        context.expect_call(
            "ECHO",
            &["TEST"],
            ValkeyValue::SimpleString("TEST".to_owned()),
        );
        context.expect_call(
            "SHUTDOWN",
            &[] as &[&str],
            ValkeyValue::StaticError("not allowed in script mode"),
        );
        context.expect_call("HSET", &["x", "foo", "bar"], ValkeyValue::Integer(1));
        context.expect_call(
            "HGETALL",
            &["x"],
            ValkeyValue::Map(HashMap::from([(
                ValkeyValueKey::String("foo".to_owned()),
                ValkeyValue::SimpleString("bar".to_owned()),
            )])),
        );

        assert!(call_test(&context, Vec::new()).is_ok());
    }

    #[test]
    fn call_test_accepts_configured_replies_through_thread_safe_context() {
        let mut context = ThreadSafeContext::test();
        context.expect_call(
            "ECHO",
            &["TEST"],
            ValkeyValue::SimpleString("TEST".to_owned()),
        );
        context.expect_call(
            "SHUTDOWN",
            &[] as &[&str],
            ValkeyValue::StaticError("not allowed in script mode"),
        );
        context.expect_call("HSET", &["x", "foo", "bar"], ValkeyValue::Integer(1));
        context.expect_call(
            "HGETALL",
            &["x"],
            ValkeyValue::Map(HashMap::from([(
                ValkeyValueKey::String("foo".to_owned()),
                ValkeyValue::SimpleString("bar".to_owned()),
            )])),
        );

        context.with_lock(|guard| {
            let result = call_test(guard, Vec::new());

            assert!(matches!(
                result,
                Ok(ValkeyValue::BulkString(value)) if value == "pass"
            ));
        });
    }
}
