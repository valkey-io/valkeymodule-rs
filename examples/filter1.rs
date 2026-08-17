use std::ops::Deref;
use std::sync::RwLock;
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::logging::log_notice;
use valkey_module::{
    valkey_module, CommandFilter, CommandFilterCtx, Context, RedisModuleCommandFilterCtx, Status,
    ValkeyResult, ValkeyString, VALKEYMODULE_CMDFILTER_NOSELF,
};

// used to register filters using init and deinit, this is more complicated, best use the macro approach below
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

// this is used to register and unregister the filter in init and deinit
// has to be extern "C", better to use the macro approach
extern "C" fn info_filter_fn(ctx: *mut RedisModuleCommandFilterCtx) {
    let cf_ctx = CommandFilterCtx::new(ctx);

    // we want the filter to be very efficient as it will be called for every command
    // check if there is only 1 arg, the command
    if cf_ctx.args_count() != 1 {
        return;
    }
    // check if cmd (first arg) is info
    let cmd = cf_ctx.arg_get_try_as_str(0).unwrap();
    if !cmd.eq_ignore_ascii_case("info") {
        return;
    }
    // grab client_id
    let client_id = cf_ctx.get_client_id();
    log_notice(&format!("info filter for client_id {}", client_id));
    // replace info with info2 as the command name
    cf_ctx.arg_replace(0, "info2");
}

// custom command that will replace info command
fn info2(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    ctx.log_notice("info2 command");
    //  do something different here
    Ok("info2\n".into())
}

// this will be registered via valkey_module! macro, can be a regular Rust fn (not extern "C").
// this is the recommended approach for registering filters as it is simpler and cleaner
fn set_filter_fn(ctx: *mut RedisModuleCommandFilterCtx) {
    let cf_ctx = CommandFilterCtx::new(ctx);

    if cf_ctx.args_count() != 3 {
        return;
    }
    // check if cmd (first arg) is set
    let cmd = cf_ctx.cmd_get_try_as_str().unwrap();
    if !cmd.eq_ignore_ascii_case("set") {
        return;
    }
    let all_args = cf_ctx.get_all_args_wo_cmd();
    log_notice(&format!("all_args: {:?}", all_args));
    let key = cf_ctx.arg_get_try_as_str(1).unwrap();
    let value = cf_ctx.arg_get_try_as_str(2).unwrap();
    log_notice(&format!("set key: {}, value {}", key, value));
    // delete 2nd arg key
    cf_ctx.arg_delete(1);
    // insert new key
    cf_ctx.arg_insert(1, "new_key");
    // replace 3rd arg value
    cf_ctx.arg_replace(2, "new_value");
}

fn filter1_fn(_ctx: *mut RedisModuleCommandFilterCtx) {
    // do something here, registered via valkey_module! macro
}

fn filter2_fn(_ctx: *mut RedisModuleCommandFilterCtx) {
    // do something here, registered via valkey_module! macro
}

valkey_module! {
    name: "filter1",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    init: init,
    deinit: deinit,
    commands: [
        ["info2", info2, "readonly", 0, 0, 0],
    ],
    filters: [
        // need to add paste crate to your Cargo.toml or you will get error:     use of undeclared crate or module `paste`
        [set_filter_fn, VALKEYMODULE_CMDFILTER_NOSELF],
        [filter1_fn, VALKEYMODULE_CMDFILTER_NOSELF],
        [filter2_fn, VALKEYMODULE_CMDFILTER_NOSELF]
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_filter_rewrites_info_command() {
        let mut context = CommandFilterCtx::test();
        context.expect_args(&["INFO"]).expect_get_client_id(42);

        info_filter_fn(context.as_raw_ctx_ptr());

        assert_eq!(context.cmd_get_try_as_str(), Ok("info2"));
    }

    #[test]
    fn info_filter_leaves_unrelated_command_unchanged() {
        let mut context = CommandFilterCtx::test();
        context.expect_args(&["PING"]);

        info_filter_fn(context.as_raw_ctx_ptr());

        assert_eq!(context.cmd_get_try_as_str(), Ok("PING"));
    }

    #[test]
    fn set_filter_rewrites_key_and_value() {
        let mut context = CommandFilterCtx::test();
        context.expect_args(&["SET", "old_key", "old_value"]);

        set_filter_fn(context.as_raw_ctx_ptr());

        assert_eq!(context.args_count(), 3);
        assert_eq!(context.cmd_get_try_as_str(), Ok("SET"));
        assert_eq!(context.arg_get_try_as_str(1), Ok("new_key"));
        assert_eq!(context.arg_get_try_as_str(2), Ok("new_value"));
    }

    #[test]
    fn set_filter_leaves_wrong_arity_unchanged() {
        let mut context = CommandFilterCtx::test();
        context.expect_args(&["SET", "key"]);

        set_filter_fn(context.as_raw_ctx_ptr());

        assert_eq!(context.args_count(), 2);
        assert_eq!(context.cmd_get_try_as_str(), Ok("SET"));
        assert_eq!(context.arg_get_try_as_str(1), Ok("key"));
    }

    #[test]
    fn set_filter_leaves_unrelated_command_unchanged() {
        let mut context = CommandFilterCtx::test();
        context.expect_args(&["GET", "key", "value"]);

        set_filter_fn(context.as_raw_ctx_ptr());

        assert_eq!(context.args_count(), 3);
        assert_eq!(context.cmd_get_try_as_str(), Ok("GET"));
        assert_eq!(context.arg_get_try_as_str(1), Ok("key"));
        assert_eq!(context.arg_get_try_as_str(2), Ok("value"));
    }
}
