use std::ffi::CString;
use std::ops::Deref;
use std::ptr::null_mut;
use std::sync::RwLock;
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::logging::log_notice;
use valkey_module::{
    valkey_module, CommandFilter, CommandFilterCtx, Context, RedisModuleCommandFilterCtx,
    RedisModuleString, RedisModule_CreateString, Status, ValkeyResult, ValkeyString,
    VALKEYMODULE_CMDFILTER_NOSELF,
};

// this shows how to register filters using init and deinit
static INFO_FILTER: RwLock<Option<CommandFilter>> = RwLock::new(None);

fn init(ctx: &Context, _args: &[ValkeyString]) -> Status {
    let info_filter = ctx.register_command_filter(info_filter_fn, VALKEYMODULE_CMDFILTER_NOSELF);
    if info_filter.is_null() {
        return Status::Err;
    }
    let mut info_guard = INFO_FILTER.write().unwrap();
    *info_guard = Some(info_filter);

    Status::Ok
}

fn deinit(ctx: &Context) -> Status {
    let info_guard = INFO_FILTER.read().unwrap();
    if let Some(ref info_filter) = info_guard.deref() {
        ctx.unregister_command_filter(info_filter);
    };

    Status::Ok
}

/// this is just an example, please don't use this in production
extern "C" fn set_filter_fn(ctx: *mut RedisModuleCommandFilterCtx) {
    let cf_ctx = CommandFilterCtx::new(ctx);

    if cf_ctx.args_count() != 3 {
        return;
    }
    // check if cmd (first arg) is set
    let cmd = cf_ctx.cmd_get_as_str();
    if !cmd.eq_ignore_ascii_case("set") {
        return;
    }
    let key = cf_ctx.arg_get_as_str(1).unwrap();
    let value = cf_ctx.arg_get_as_str(2).unwrap();
    log_notice(&format!("set key: {}, value {}", key, value));
    // delete 2nd arg key
    cf_ctx.arg_delete(1);
    // insert new key
    let new_key = create_module_string("new_key");
    cf_ctx.arg_insert(1, new_key);
    // replace 3rd arg value
    let new_value = create_module_string("new_value");
    cf_ctx.arg_replace(2, new_value);
}

extern "C" fn info_filter_fn(ctx: *mut RedisModuleCommandFilterCtx) {
    let cf_ctx = CommandFilterCtx::new(ctx);

    // we want the filter to be very efficient as it will be called for every command
    // check if there is only 1 arg, the command
    if cf_ctx.args_count() != 1 {
        return;
    }
    // check if cmd (first arg) is info
    let cmd = cf_ctx.arg_get_as_str(0).unwrap();
    if !cmd.eq_ignore_ascii_case("info") {
        return;
    }
    // grab client_id
    let client_id = cf_ctx.get_client_id();
    log_notice(&format!("info filter for client_id {}", client_id));
    // replace info with info2 as the command name
    let custom_cmd = create_module_string("info2");
    cf_ctx.arg_replace(0, custom_cmd);
}

// custom command that will be called instead of info
fn info2(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    ctx.log_notice("info2 command");
    //  do something different here
    Ok("info2".into())
}

/// create a RedisModuleString from a &str without Context which is not present in filter functions
fn create_module_string(arg: &str) -> *mut RedisModuleString {
    let arg_cstring = CString::new(arg).unwrap();
    let arg_module_string = unsafe {
        RedisModule_CreateString.unwrap()(
            null_mut(),
            arg_cstring.as_ptr(),
            arg_cstring.as_bytes().len(),
        )
    };
    arg_module_string
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
    // this shows how to register filters using valkey_module! macro
    filters: [
        [set_filter_fn, VALKEYMODULE_CMDFILTER_NOSELF]
    ],
}
