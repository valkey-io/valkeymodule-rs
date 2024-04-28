use redis_module_macros::info_command_handler;
use valkey_module::{valkey_module, ValkeyResult};
use valkey_module::{InfoContext, Status};

#[info_command_handler]
fn add_info(ctx: &InfoContext, _for_crash_report: bool) -> ValkeyResult<()> {
    if ctx.add_info_section(Some("info")) == Status::Ok {
        ctx.add_info_field_str("field", "value");
    }

    Ok(())
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "info_handler_macro",
    version: 1,
    allocator: (valkey_module::alloc::ValkeyAlloc, valkey_module::alloc::ValkeyAlloc),
    data_types: [],
    commands: [],
}
