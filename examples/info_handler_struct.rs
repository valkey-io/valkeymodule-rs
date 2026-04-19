use std::collections::HashMap;

use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{valkey_module, InfoContext, InfoContextTrait, ValkeyResult};
use valkey_module_macros::{info_command_handler, InfoSection};

#[derive(Debug, Clone, InfoSection)]
struct Info {
    field: String,
    dictionary: HashMap<String, String>,
}

// handler logic written against InfoContextTrait so it is mockable in unit tests
fn add_info_logic(ctx: &impl InfoContextTrait) -> ValkeyResult<()> {
    let mut dictionary = HashMap::new();
    dictionary.insert("key".to_owned(), "value".into());
    let data = Info {
        field: "value".to_owned(),
        dictionary,
    };
    ctx.build_one_section(data.into())
}

#[info_command_handler]
fn add_info(ctx: &InfoContext, _for_crash_report: bool) -> ValkeyResult<()> {
    add_info_logic(ctx)
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "info_handler_struct",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [],
}

#[cfg(test)]
mod tests {
    use super::*;
    use valkey_module::MockInfoContext;

    #[test]
    fn test_add_info_logic_builds_expected_section() {
        let mut ctx = MockInfoContext::new();
        ctx.expect_build_one_section()
            .withf(|(name, fields)| {
                // section name is derived from the struct name by the InfoSection derive
                name == "Info"
                    && fields.iter().any(|(field_name, _)| field_name == "field")
                    && fields
                        .iter()
                        .any(|(field_name, _)| field_name == "dictionary")
            })
            .times(1)
            .returning(|_| Ok(()));

        add_info_logic(&ctx).unwrap();
    }
}
