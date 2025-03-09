use std::ffi::{c_int, CString};
use std::ptr::null_mut;
use std::sync::RwLock;
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::logging::log_notice;
use valkey_module::{
    valkey_module, CommandFilter, Context, RedisModuleCommandFilterCtx, RedisModuleString,
    RedisModule_CreateString, Status, ValkeyResult, ValkeyString, VALKEYMODULE_CMDFILTER_NOSELF,
};

static INFO_FILTER: RwLock<Option<CommandFilter>> = RwLock::new(None);
static SET_FILTER: RwLock<Option<CommandFilter>> = RwLock::new(None);

fn init(ctx: &Context, _args: &[ValkeyString]) -> Status {
    let info_filter =
        ctx.register_command_filter(info_filter_fn, VALKEYMODULE_CMDFILTER_NOSELF as c_int);
    if info_filter.is_null() {
        return Status::Err;
    }
    let mut info_guard = INFO_FILTER.write().unwrap();
    *info_guard = Some(info_filter);

    let set_filter =
        ctx.register_command_filter(set_filter_fn, VALKEYMODULE_CMDFILTER_NOSELF as c_int);
    if set_filter.is_null() {
        return Status::Err;
    }
    let mut set_guard = SET_FILTER.write().unwrap();
    *set_guard = Some(set_filter);

    Status::Ok
}

fn deinit(ctx: &Context) -> Status {
    let info_guard = INFO_FILTER.read().unwrap();
    if let Some(ref info_filter) = *info_guard {
        ctx.unregister_command_filter(&info_filter);
    };

    let set_guard = SET_FILTER.read().unwrap();
    if let Some(ref set_filter) = *set_guard {
        ctx.unregister_command_filter(&set_filter);
    };

    Status::Ok
}

/// this is just an example, please don't use this in production
extern "C" fn set_filter_fn(ctx: *mut RedisModuleCommandFilterCtx) {
    if CommandFilter::args_count(ctx) != 3 {
        return;
    }
    // check if cmd (first arg) is set
    let cmd = CommandFilter::arg_get_as_str(ctx, 0).unwrap().to_string();
    if !cmd.eq_ignore_ascii_case("set") {
        return;
    }
    // delete 2nd arg key
    CommandFilter::arg_delete(ctx, 1);
    // insert new key
    let new_key = create_module_string("new_key");
    CommandFilter::arg_insert(ctx, 1, new_key);
    // replace 3rd arg value
    let new_value = create_module_string("new_value");
    CommandFilter::arg_replace(ctx, 2, new_value);
}

extern "C" fn info_filter_fn(ctx: *mut RedisModuleCommandFilterCtx) {
    // we want the filter to be very efficient as it will be called for every command
    // check if there is only 1 arg, the command
    let argc = CommandFilter::args_count(ctx);
    if argc != 1 {
        return;
    }
    // check if cmd (first arg) is info
    let cmd = CommandFilter::arg_get_as_str(ctx, 0).unwrap().to_string();
    if !cmd.eq_ignore_ascii_case("info") {
        return;
    }
    // grab client_id
    let client_id = CommandFilter::get_client_id(ctx);
    log_notice(&format!("info filter for client_id {}", client_id));
    // replace info with info2 as the command name
    let custom_cmd = create_module_string("info2");
    CommandFilter::arg_replace(ctx, 0, custom_cmd);
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
}
