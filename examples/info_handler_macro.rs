use valkey_module::InfoContext;
use valkey_module::{valkey_module, ValkeyResult};
use valkey_module_macros::info_command_handler;

#[info_command_handler]
fn add_info(ctx: &InfoContext, _for_crash_report: bool) -> ValkeyResult<()> {
    ctx.builder()
        .add_section("info")
        .field("field", "value")?
        .build_section()?
        .build_info()
        .map(|_| ())
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "info_handler_macro",
    version: 1,
    allocator: (valkey_module::alloc::ValkeyAlloc, valkey_module::alloc::ValkeyAlloc),
    data_types: [],
    commands: [],
}
