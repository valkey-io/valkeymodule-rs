use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{valkey_module, Context, ValkeyError, ValkeyResult, ValkeyString, ValkeyValue};

/// Concatenates the raw bytes of all arguments after the command name and
/// echoes them back via [`Context::reply_with_valkey_string`], using
/// [`Context::create_string_from_slice`] to materialize the [`ValkeyString`]
/// without going through `CString::new`. Round-trips bytes containing NUL.
fn echo_via_string(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() < 2 {
        return Err(ValkeyError::WrongArity);
    }
    let mut buf = Vec::new();
    for arg in args.iter().skip(1) {
        buf.extend_from_slice(arg.as_slice());
    }
    let s = ctx.create_string_from_slice(&buf);
    ctx.reply_with_valkey_string(&s);
    Ok(ValkeyValue::NoReply)
}

/// Concatenates the raw bytes of all arguments after the command name and
/// echoes them back via [`Context::reply_with_slice`], without ever
/// constructing a [`ValkeyString`] or [`ValkeyValue`].
fn echo_via_slice(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() < 2 {
        return Err(ValkeyError::WrongArity);
    }
    let mut buf = Vec::new();
    for arg in args.iter().skip(1) {
        buf.extend_from_slice(arg.as_slice());
    }
    ctx.reply_with_slice(&buf);
    Ok(ValkeyValue::NoReply)
}

valkey_module! {
    name: "borrowed_strings",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["borrowed_strings.echo_string", echo_via_string, "readonly fast", 0, 0, 0],
        ["borrowed_strings.echo_slice", echo_via_slice, "readonly fast", 0, 0, 0],
    ],
}
