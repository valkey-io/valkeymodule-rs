use std::sync::{LazyLock, Mutex};
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{
    logging::ValkeyLogLevel, valkey_module, Context, Status, ValkeyResult, ValkeyString,
    ValkeyValue,
};

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

    // The `log` crate logger can only be set once per process. It is still set
    // here on a reload if the server did not unmap the module on unload.
    if let Err(e) = valkey_module::logging::setup() {
        ctx.log(ValkeyLogLevel::Warning, &format!("Logger not set: {e}"));
    }
    log::info!("load_unload loaded");

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

fn log_message(_ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    let message = args.into_iter().skip(1).collect::<Vec<_>>().join(" ");
    log::info!("{message}");
    Ok(ValkeyValue::SimpleStringStatic("OK"))
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "load_unload",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    init: init,
    deinit: deinit,
    commands: [
        ["load_unload.log", log_message, "", 0, 0, 0],
    ],
}
