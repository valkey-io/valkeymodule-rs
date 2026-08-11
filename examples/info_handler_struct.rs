use std::collections::HashMap;

use valkey_module::alloc::ValkeyAlloc;
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
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [],
}

#[cfg(test)]
mod tests {
    use super::*;
    use valkey_module::test_shims::{TestInfoEntry, TestInfoField, TestInfoSection, TestInfoValue};

    #[test]
    fn captures_derived_info_struct_and_dictionary() {
        let info_ctx = InfoContext::test();

        assert!(add_info(&info_ctx, false).is_ok());

        assert_eq!(
            info_ctx.sections(),
            vec![TestInfoSection {
                name: Some("Info".to_owned()),
                entries: vec![
                    TestInfoEntry::Field(TestInfoField {
                        name: "field".to_owned(),
                        value: TestInfoValue::String("value".to_owned()),
                    }),
                    TestInfoEntry::Dictionary {
                        name: "dictionary".to_owned(),
                        fields: vec![TestInfoField {
                            name: "key".to_owned(),
                            value: TestInfoValue::String("value".to_owned()),
                        }],
                    },
                ],
            }]
        );
    }

    #[test]
    fn skips_unrequested_info_section() {
        let mut info_ctx = InfoContext::test();
        info_ctx.expect_unrequested_section("Info");

        assert!(add_info(&info_ctx, false).is_ok());
        assert!(info_ctx.sections().is_empty());
    }
}
