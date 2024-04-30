use valkey_module::{valkey_module, Context, ValkeyError, ValkeyResult, ValkeyString};
use valkey_module::{InfoContext, Status};

fn test_helper_version(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    let ver = ctx.get_redis_version()?;
    let response: Vec<i64> = vec![ver.major.into(), ver.minor.into(), ver.patch.into()];

    Ok(response.into())
}

fn test_helper_version_rm_call(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    let ver = ctx.get_redis_version_rm_call()?;
    let response: Vec<i64> = vec![ver.major.into(), ver.minor.into(), ver.patch.into()];

    Ok(response.into())
}

fn test_helper_command_name(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ctx.current_command_name()?.into())
}

fn test_helper_err(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() < 1 {
        return Err(ValkeyError::WrongArity);
    }

    let msg = args.get(1).unwrap();

    ctx.reply_error_string(msg.try_as_str().unwrap());
    Ok(().into())
}

fn add_info(ctx: &InfoContext, _for_crash_report: bool) {
    if ctx.add_info_section(Some("test_helper")) == Status::Ok {
        ctx.add_info_field_str("field", "value");
    }
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "test_helper",
    version: 1,
    allocator: (valkey_module::alloc::ValkeyAlloc, valkey_module::alloc::ValkeyAlloc),
    data_types: [],
    info: add_info,
    commands: [
        ["test_helper.version", test_helper_version, "", 0, 0, 0],
        ["test_helper._version_rm_call", test_helper_version_rm_call, "", 0, 0, 0],
        ["test_helper.name", test_helper_command_name, "", 0, 0, 0],
        ["test_helper.err", test_helper_err, "", 0, 0, 0],
    ],
}
