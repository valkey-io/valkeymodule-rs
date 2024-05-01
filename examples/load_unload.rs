use valkey_module::{logging::ValkeyLogLevel, valkey_module, Context, Status, ValkeyString};

static mut GLOBAL_STATE: Option<String> = None;

fn init(ctx: &Context, args: &[ValkeyString]) -> Status {
    let (before, after) = unsafe {
        let before = GLOBAL_STATE.clone();
        GLOBAL_STATE.replace(format!("Args passed: {}", args.join(", ")));
        let after = GLOBAL_STATE.clone();
        (before, after)
    };
    ctx.log(
        ValkeyLogLevel::Warning,
        &format!("Update global state on LOAD. BEFORE: {before:?}, AFTER: {after:?}",),
    );

    Status::Ok
}

fn deinit(ctx: &Context) -> Status {
    let (before, after) = unsafe {
        let before = GLOBAL_STATE.take();
        let after = GLOBAL_STATE.clone();
        (before, after)
    };
    ctx.log(
        ValkeyLogLevel::Warning,
        &format!("Update global state on UNLOAD. BEFORE: {before:?}, AFTER: {after:?}"),
    );

    Status::Ok
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "load_unload",
    version: 1,
    allocator: (valkey_module::alloc::ValkeyAlloc, valkey_module::alloc::ValkeyAlloc),
    data_types: [],
    init: init,
    deinit: deinit,
    commands: [],
}
