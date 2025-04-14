use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{valkey_module, Context, Status, ValkeyString};

fn preload(ctx: &Context, args: &[ValkeyString]) -> Status {
    // perform preload validations here, useful for MODULE LOAD
    // unlike init which is called at the end of the valkey_module! macro this is called at the beginning
    let version = ctx.get_server_version().unwrap();
    ctx.log_notice(&format!(
        "preload for server version {:?} with args: {:?}",
        version, args
    ));
    // respond with either Status::Ok or Status::Err (if you want to prevent module loading)
    Status::Ok
}

valkey_module! {
    name: "preload",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    preload: preload,
    commands: [],
}
