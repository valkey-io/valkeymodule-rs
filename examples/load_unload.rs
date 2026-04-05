use std::sync::{LazyLock, Mutex};
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{logging::ValkeyLogLevel, valkey_module, Context, Status, ValkeyString};

static GLOBAL_STATE: LazyLock<Mutex<Option<String>>> = LazyLock::new(|| Mutex::new(None));

fn init(ctx: &Context, args: &[ValkeyString]) -> Status {
    let (before, after) = {
        let mut state = GLOBAL_STATE.lock().unwrap();
        let before = state.clone();
        state.replace(format!("Args passed: {}", args.join(", ")));
        let after = state.clone();
        (before, after)
    };
    ctx.log(
        ValkeyLogLevel::Warning,
        &format!("Update global state on LOAD. BEFORE: {before:?}, AFTER: {after:?}",),
    );

    Status::Ok
}

fn deinit(ctx: &Context) -> Status {
    let (before, after) = {
        let mut state = GLOBAL_STATE.lock().unwrap();
        let before = state.take();
        let after = state.clone();
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
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    init: init,
    deinit: deinit,
    commands: [],
}
