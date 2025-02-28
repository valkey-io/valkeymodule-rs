use std::ffi::c_int;
use std::sync::RwLock;
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::context::filter::*;
use valkey_module::logging::log_notice;
use valkey_module::{
    valkey_module, Context, RedisModuleCommandFilterCtx, Status, ValkeyResult, ValkeyString,
    VALKEYMODULE_CMDFILTER_NOSELF,
};

static INFO_FILTER: RwLock<Option<CommandFilter>> = RwLock::new(None);

fn init(ctx: &Context, _args: &[ValkeyString]) -> Status {
    let info_filter =
        ctx.register_command_filter(info_filter_fn, VALKEYMODULE_CMDFILTER_NOSELF as c_int);
    if info_filter.inner.is_null() {
        return Status::Err;
    }
    ctx.log_notice(&format!("filter init {:?}", &info_filter));
    let mut guard = INFO_FILTER.write().unwrap();
    *guard = Some(info_filter);
    Status::Ok
}

fn deinit(ctx: &Context) -> Status {
    let guard = INFO_FILTER.read().unwrap();
    if let Some(ref info_filter) = *guard {
        ctx.unregister_command_filter(&info_filter);
        ctx.log_notice(&format!("filter deinit {:?}", info_filter));
        Status::Ok
    } else {
        Status::Err
    }
}
/// custom filter function that will fire before info command
extern "C" fn info_filter_fn(ctx: *mut RedisModuleCommandFilterCtx) {
    // check if there is only 1 arg, the command
    let argc = command_filter_args_count(ctx);
    if argc != 1 {
        return;
    }
    // check if cmd (first arg) is info
    let cmd = command_filter_arg_get(ctx, 0);
    let cmd_str = ValkeyString::from_ptr(cmd).unwrap();
    if !cmd_str.eq_ignore_ascii_case("info") {
        return;
    }
    log_notice("info filter");
    // replace info with info2 as the command name
    let custom_cmd = arg_module_create_string("info2");
    command_filter_arg_replace(ctx, 0, custom_cmd);
}

// custom command that will be called instead of info
fn info2(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    ctx.log_notice("info2 command");
    //  do something different here or just call real command
    let _resp = ctx.call("info", &[""])?;
    Ok("info2".into())
}

valkey_module! {
    name: "filter",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    init: init,
    deinit: deinit,
    commands: [
        ["info2", info2, "readonly", 0, 0, 0],
    ],
}
