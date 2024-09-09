use valkey_module::alloc::ValkeyAlloc;
use valkey_module::InfoContext;
use valkey_module::{valkey_module, ValkeyResult};
use valkey_module_macros::info_command_handler;

#[info_command_handler]
fn add_info(ctx: &InfoContext, _for_crash_report: bool) -> ValkeyResult<()> {
    ctx.builder()
        .add_section("info")
        .field("field", "value")?
        .add_dictionary("dictionary")
        .field("key", "value")?
        .build_dictionary()?
        .build_section()?
        .build_info()?;

    Ok(())
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "info_handler_builder",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [],
}
