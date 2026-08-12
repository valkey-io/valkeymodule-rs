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

#[cfg(test)]
mod tests {
    use super::*;
    use valkey_module::test_shims::{TestInfoEntry, TestInfoField, TestInfoSection, TestInfoValue};

    #[test]
    fn captures_builder_info_and_dictionary() {
        let info_ctx = InfoContext::test();

        assert!(add_info(&info_ctx, false).is_ok());

        assert_eq!(
            info_ctx.sections(),
            vec![TestInfoSection {
                name: Some("info".to_owned()),
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
        info_ctx.expect_unrequested_section("info");

        assert!(add_info(&info_ctx, false).is_ok());
        assert!(info_ctx.sections().is_empty());
    }
}
