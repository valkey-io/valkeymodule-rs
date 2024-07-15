use std::collections::HashMap;

use valkey_module::InfoContext;
use valkey_module::{valkey_module, ValkeyResult};
use valkey_module_macros::{info_command_handler, InfoSection};

#[derive(Debug, Clone, InfoSection)]
struct Info {
    field: String,
    dictionary: HashMap<String, String>,
}

#[info_command_handler]
fn add_info(ctx: &InfoContext, _for_crash_report: bool) -> ValkeyResult<()> {
    let mut dictionary = HashMap::new();
    dictionary.insert("key".to_owned(), "value".into());
    let data = Info {
        field: "value".to_owned(),
        dictionary,
    };
    ctx.build_one_section(data)
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "info_handler_struct",
    version: 1,
    allocator: (valkey_module::alloc::ValkeyAlloc, valkey_module::alloc::ValkeyAlloc),
    data_types: [],
    commands: [],
}
