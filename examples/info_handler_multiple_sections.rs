use valkey_module::alloc::ValkeyAlloc;
use valkey_module::InfoContext;
use valkey_module::{valkey_module, ValkeyResult};
use valkey_module_macros::{info_command_handler, InfoSection};

#[derive(Debug, Clone, InfoSection)]
struct InfoSection1 {
    field_1: String,
}

#[derive(Debug, Clone, InfoSection)]
struct InfoSection2 {
    field_2: String,
}

#[info_command_handler]
fn add_info(ctx: &InfoContext, _for_crash_report: bool) -> ValkeyResult<()> {
    let data = InfoSection1 {
        field_1: "value1".to_owned(),
    };
    let _ = ctx.build_one_section(data)?;

    let data = InfoSection2 {
        field_2: "value2".to_owned(),
    };

    ctx.build_one_section(data)
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "info_handler_multiple_sections",
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
    fn captures_both_derived_info_sections_in_order() {
        let info_ctx = InfoContext::test();

        assert!(add_info(&info_ctx, false).is_ok());

        assert_eq!(
            info_ctx.sections(),
            vec![
                TestInfoSection {
                    name: Some("InfoSection1".to_owned()),
                    entries: vec![TestInfoEntry::Field(TestInfoField {
                        name: "field_1".to_owned(),
                        value: TestInfoValue::String("value1".to_owned()),
                    })],
                },
                TestInfoSection {
                    name: Some("InfoSection2".to_owned()),
                    entries: vec![TestInfoEntry::Field(TestInfoField {
                        name: "field_2".to_owned(),
                        value: TestInfoValue::String("value2".to_owned()),
                    })],
                },
            ]
        );
    }

    #[test]
    fn skips_unrequested_first_info_section() {
        let mut info_ctx = InfoContext::test();
        info_ctx.expect_unrequested_section("InfoSection1");

        assert!(add_info(&info_ctx, false).is_ok());
        assert_eq!(
            info_ctx.sections(),
            vec![TestInfoSection {
                name: Some("InfoSection2".to_owned()),
                entries: vec![TestInfoEntry::Field(TestInfoField {
                    name: "field_2".to_owned(),
                    value: TestInfoValue::String("value2".to_owned()),
                })],
            }]
        );
    }
}
