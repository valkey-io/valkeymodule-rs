use crate::{raw, InfoContext};
use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::CStr;
use std::ops::Deref;

/// A typed value captured from an INFO field.
#[derive(Debug, Clone, PartialEq)]
pub enum TestInfoValue {
    String(String),
    I64(i64),
    U64(u64),
    F64(f64),
}

/// A named INFO field captured by a test context.
#[derive(Debug, Clone, PartialEq)]
pub struct TestInfoField {
    /// The field name passed to the module API.
    pub name: String,
    /// The typed value emitted for the field.
    pub value: TestInfoValue,
}

/// An entry captured within an INFO section.
#[derive(Debug, Clone, PartialEq)]
pub enum TestInfoEntry {
    /// A scalar field.
    Field(TestInfoField),
    /// A dictionary and its captured fields.
    Dictionary {
        /// The dictionary name passed to the module API.
        name: String,
        /// The dictionary fields in emission order.
        fields: Vec<TestInfoField>,
    },
}

/// An INFO section captured by a test context.
#[derive(Debug, Clone, PartialEq)]
pub struct TestInfoSection {
    /// The section name, or `None` for an unnamed section.
    pub name: Option<String>,
    /// The section entries in emission order.
    pub entries: Vec<TestInfoEntry>,
}

#[derive(Default)]
struct InfoContextData {
    sections: Vec<TestInfoSection>,
    current_section: Option<usize>,
    current_dictionary: Option<usize>,
    // Sections Valkey would reject as not requested, causing `InfoAddSection` to return `ERR`.
    unrequested_sections: HashSet<String>,
}

thread_local! {
    static TEST_INFO_CONTEXTS: RefCell<HashSet<usize>> = RefCell::default();
}

/// Owns a test-only [`InfoContext`] that captures emitted INFO data.
pub struct TestInfoContext {
    context: InfoContext,
    data: Box<InfoContextData>,
}

impl InfoContext {
    /// Creates a test INFO context that can be used without a running Valkey server.
    #[must_use]
    pub fn test() -> TestInfoContext {
        TestInfoContext::new()
    }
}

impl TestInfoContext {
    fn new() -> Self {
        super::setup_test_shims();

        let mut data = Box::<InfoContextData>::default();
        let ctx = (&mut *data as *mut InfoContextData).cast::<raw::RedisModuleInfoCtx>();
        TEST_INFO_CONTEXTS.with(|contexts| {
            contexts.borrow_mut().insert(ctx as usize);
        });

        Self {
            context: InfoContext::new(ctx),
            data,
        }
    }

    /// Returns an owned snapshot of the INFO sections captured so far.
    #[must_use]
    pub fn sections(&self) -> Vec<TestInfoSection> {
        self.data.sections.clone()
    }

    /// Configures a section as unrequested, causing `InfoAddSection` to return `ERR`.
    pub fn expect_unrequested_section(&mut self, name: impl Into<String>) -> &mut Self {
        self.data.unrequested_sections.insert(name.into());
        self
    }
}

impl Deref for TestInfoContext {
    type Target = InfoContext;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

impl Drop for TestInfoContext {
    fn drop(&mut self) {
        TEST_INFO_CONTEXTS.with(|contexts| {
            contexts.borrow_mut().remove(&(self.context.ctx as usize));
        });
    }
}

fn with_data_mut<T>(
    ctx: *mut raw::RedisModuleInfoCtx,
    operation: impl FnOnce(&mut InfoContextData) -> T,
) -> Option<T> {
    if ctx.is_null()
        || !TEST_INFO_CONTEXTS.with(|contexts| contexts.borrow().contains(&(ctx as usize)))
    {
        return None;
    }

    // SAFETY: the registry contains only live `InfoContextData` allocations, and INFO callbacks
    // execute synchronously on one thread.
    Some(operation(unsafe { &mut *ctx.cast::<InfoContextData>() }))
}

fn required_c_string(value: *const libc::c_char) -> Option<String> {
    if value.is_null() {
        return None;
    }

    // SAFETY: Module API callbacks require a NUL-terminated input string.
    Some(
        unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned(),
    )
}

fn append_field(
    ctx: *mut raw::RedisModuleInfoCtx,
    name: *const libc::c_char,
    value: TestInfoValue,
) -> libc::c_int {
    let Some(name) = required_c_string(name) else {
        return raw::Status::Err as libc::c_int;
    };

    with_data_mut(ctx, |data| {
        let Some(section_index) = data.current_section else {
            return raw::Status::Err as libc::c_int;
        };
        let field = TestInfoField { name, value };
        if let Some(dictionary_index) = data.current_dictionary {
            let Some(TestInfoEntry::Dictionary { fields, .. }) = data.sections[section_index]
                .entries
                .get_mut(dictionary_index)
            else {
                return raw::Status::Err as libc::c_int;
            };
            fields.push(field);
        } else {
            data.sections[section_index]
                .entries
                .push(TestInfoEntry::Field(field));
        }
        raw::Status::Ok as libc::c_int
    })
    .unwrap_or(raw::Status::Err as libc::c_int)
}

pub(super) extern "C" fn info_add_section(
    ctx: *mut raw::RedisModuleInfoCtx,
    name: *const libc::c_char,
) -> libc::c_int {
    let name = if name.is_null() {
        None
    } else {
        let Some(name) = required_c_string(name) else {
            return raw::Status::Err as libc::c_int;
        };
        Some(name)
    };

    with_data_mut(ctx, |data| {
        if data.current_dictionary.is_some() {
            return raw::Status::Err as libc::c_int;
        }
        if name
            .as_ref()
            .is_some_and(|name| data.unrequested_sections.contains(name))
        {
            data.current_section = None;
            data.current_dictionary = None;
            return raw::Status::Err as libc::c_int;
        }

        data.sections.push(TestInfoSection {
            name,
            entries: Vec::new(),
        });
        data.current_section = Some(data.sections.len() - 1);
        raw::Status::Ok as libc::c_int
    })
    .unwrap_or(raw::Status::Err as libc::c_int)
}

pub(super) extern "C" fn info_add_field_string(
    ctx: *mut raw::RedisModuleInfoCtx,
    field: *const libc::c_char,
    value: *mut raw::RedisModuleString,
) -> libc::c_int {
    if value.is_null() {
        return raw::Status::Err as libc::c_int;
    }

    // SAFETY: the test shim creates and owns this opaque module string for the duration of the
    // synchronous callback.
    let value = unsafe { super::valkey_string::string_data(value) };
    let Ok(value) = String::from_utf8(value.to_vec()) else {
        return raw::Status::Err as libc::c_int;
    };
    append_field(ctx, field, TestInfoValue::String(value))
}

pub(super) extern "C" fn info_add_field_long_long(
    ctx: *mut raw::RedisModuleInfoCtx,
    field: *const libc::c_char,
    value: libc::c_longlong,
) -> libc::c_int {
    append_field(ctx, field, TestInfoValue::I64(value))
}

pub(super) extern "C" fn info_add_field_unsigned_long_long(
    ctx: *mut raw::RedisModuleInfoCtx,
    field: *const libc::c_char,
    value: libc::c_ulonglong,
) -> libc::c_int {
    append_field(ctx, field, TestInfoValue::U64(value))
}

pub(super) extern "C" fn info_add_field_double(
    ctx: *mut raw::RedisModuleInfoCtx,
    field: *const libc::c_char,
    value: libc::c_double,
) -> libc::c_int {
    append_field(ctx, field, TestInfoValue::F64(value))
}

pub(super) extern "C" fn info_begin_dict_field(
    ctx: *mut raw::RedisModuleInfoCtx,
    name: *const libc::c_char,
) -> libc::c_int {
    let Some(name) = required_c_string(name) else {
        return raw::Status::Err as libc::c_int;
    };

    with_data_mut(ctx, |data| {
        let Some(section_index) = data.current_section else {
            return raw::Status::Err as libc::c_int;
        };
        if data.current_dictionary.is_some() {
            return raw::Status::Err as libc::c_int;
        }

        let dictionary_index = data.sections[section_index].entries.len();
        data.sections[section_index]
            .entries
            .push(TestInfoEntry::Dictionary {
                name,
                fields: Vec::new(),
            });
        data.current_dictionary = Some(dictionary_index);
        raw::Status::Ok as libc::c_int
    })
    .unwrap_or(raw::Status::Err as libc::c_int)
}

pub(super) extern "C" fn info_end_dict_field(ctx: *mut raw::RedisModuleInfoCtx) -> libc::c_int {
    with_data_mut(ctx, |data| {
        if data.current_dictionary.take().is_some() {
            raw::Status::Ok as libc::c_int
        } else {
            raw::Status::Err as libc::c_int
        }
    })
    .unwrap_or(raw::Status::Err as libc::c_int)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        InfoContext, InfoContextBuilderFieldBottomLevelValue, InfoContextBuilderFieldTopLevelValue,
        Status,
    };
    use std::ffi::CString;

    #[test]
    fn captures_all_scalar_field_types() {
        let info = InfoContext::test();

        info.builder()
            .add_section("metrics")
            .field("text", "ready")
            .expect("text field should be unique")
            .field("signed", -7_i64)
            .expect("signed field should be unique")
            .field("unsigned", 8_u64)
            .expect("unsigned field should be unique")
            .field("ratio", InfoContextBuilderFieldBottomLevelValue::F64(1.25))
            .expect("ratio field should be unique")
            .build_section()
            .expect("section should be unique")
            .build_info()
            .expect("shim should accept INFO fields");

        assert_eq!(
            info.sections(),
            vec![TestInfoSection {
                name: Some("metrics".to_owned()),
                entries: vec![
                    TestInfoEntry::Field(TestInfoField {
                        name: "text".to_owned(),
                        value: TestInfoValue::String("ready".to_owned()),
                    }),
                    TestInfoEntry::Field(TestInfoField {
                        name: "signed".to_owned(),
                        value: TestInfoValue::I64(-7),
                    }),
                    TestInfoEntry::Field(TestInfoField {
                        name: "unsigned".to_owned(),
                        value: TestInfoValue::U64(8),
                    }),
                    TestInfoEntry::Field(TestInfoField {
                        name: "ratio".to_owned(),
                        value: TestInfoValue::F64(1.25),
                    }),
                ],
            }]
        );

        #[allow(deprecated)]
        {
            let direct = InfoContext::test();
            assert_eq!(direct.add_info_section(None), Status::Ok);
            assert_eq!(direct.add_info_field_str("state", "ok"), Status::Ok);
            assert_eq!(direct.add_info_field_long_long("count", -2), Status::Ok);
            assert_eq!(direct.sections()[0].name, None);
        }
    }

    #[test]
    fn captures_build_one_section_and_preserves_section_order() {
        let info = InfoContext::test();
        info.build_one_section((
            "first".to_owned(),
            vec![(
                "value".to_owned(),
                InfoContextBuilderFieldTopLevelValue::from(1_i64),
            )],
        ))
        .expect("first section should build");
        info.build_one_section((
            "second".to_owned(),
            vec![(
                "value".to_owned(),
                InfoContextBuilderFieldTopLevelValue::from(2_u64),
            )],
        ))
        .expect("second section should build");

        assert_eq!(
            info.sections()
                .iter()
                .map(|section| section.name.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("first"), Some("second")]
        );
    }

    #[test]
    fn test_info_contexts_do_not_share_output() {
        let first = InfoContext::test();
        first
            .build_one_section((
                "first".to_owned(),
                vec![(
                    "value".to_owned(),
                    InfoContextBuilderFieldTopLevelValue::from(1_i64),
                )],
            ))
            .expect("first context should capture output");

        let second = InfoContext::test();
        assert!(second.sections().is_empty());
        assert_eq!(first.sections().len(), 1);
    }

    #[test]
    fn callbacks_reject_invalid_context() {
        assert_eq!(
            info_add_section(std::ptr::null_mut(), std::ptr::null()),
            Status::Err as libc::c_int
        );
    }

    #[test]
    fn dropped_context_pointer_is_rejected() {
        let stale = {
            let info = InfoContext::test();
            info.ctx
        };
        assert_eq!(
            info_add_section(stale, std::ptr::null()),
            Status::Err as libc::c_int
        );
    }

    #[test]
    fn captures_dictionary_fields() {
        let info = InfoContext::test();

        info.builder()
            .add_section("keyspace")
            .add_dictionary("db0")
            .field("keys", 12_u64)
            .expect("keys field should be unique")
            .field("expires", 3_i64)
            .expect("expires field should be unique")
            .field("status", "ready")
            .expect("status field should be unique")
            .field("ratio", InfoContextBuilderFieldBottomLevelValue::F64(0.25))
            .expect("ratio field should be unique")
            .build_dictionary()
            .expect("dictionary should build")
            .build_section()
            .expect("section should build")
            .build_info()
            .expect("INFO data should build");

        assert_eq!(
            info.sections()[0].entries,
            vec![TestInfoEntry::Dictionary {
                name: "db0".to_owned(),
                fields: vec![
                    TestInfoField {
                        name: "keys".to_owned(),
                        value: TestInfoValue::U64(12),
                    },
                    TestInfoField {
                        name: "expires".to_owned(),
                        value: TestInfoValue::I64(3),
                    },
                    TestInfoField {
                        name: "status".to_owned(),
                        value: TestInfoValue::String("ready".to_owned()),
                    },
                    TestInfoField {
                        name: "ratio".to_owned(),
                        value: TestInfoValue::F64(0.25),
                    },
                ],
            }]
        );
    }

    #[test]
    fn skips_sections_configured_as_unrequested() {
        let mut info = InfoContext::test();
        info.expect_unrequested_section("hidden");

        info.builder()
            .add_section("hidden")
            .field("ignored", 1_i64)
            .expect("ignored field should be unique")
            .build_section()
            .expect("section definition should be valid")
            .add_section("visible")
            .field("kept", 2_i64)
            .expect("kept field should be unique")
            .build_section()
            .expect("section definition should be valid")
            .build_info()
            .expect("unrequested sections should be skipped");

        assert_eq!(info.sections().len(), 1);
        assert_eq!(info.sections()[0].name.as_deref(), Some("visible"));
    }

    #[test]
    fn rejects_field_before_section() {
        let info_ctx = InfoContext::test();
        let field = CString::new("field").expect("field name should not contain NUL");

        assert_eq!(
            info_add_field_long_long(info_ctx.ctx, field.as_ptr(), 1),
            Status::Err as libc::c_int
        );
        assert!(info_ctx.sections().is_empty());
    }

    #[test]
    fn rejects_dictionary_before_section() {
        let info_ctx = InfoContext::test();
        let dictionary =
            CString::new("dictionary").expect("dictionary name should not contain NUL");

        assert_eq!(
            info_begin_dict_field(info_ctx.ctx, dictionary.as_ptr()),
            Status::Err as libc::c_int
        );
        assert!(info_ctx.sections().is_empty());
    }

    #[test]
    fn rejects_nested_dictionary() {
        let info_ctx = InfoContext::test();
        let section = CString::new("section").expect("section name should not contain NUL");
        let outer = CString::new("outer").expect("dictionary name should not contain NUL");
        let nested = CString::new("nested").expect("dictionary name should not contain NUL");

        assert_eq!(
            info_add_section(info_ctx.ctx, section.as_ptr()),
            Status::Ok as libc::c_int
        );
        assert_eq!(
            info_begin_dict_field(info_ctx.ctx, outer.as_ptr()),
            Status::Ok as libc::c_int
        );
        assert_eq!(
            info_begin_dict_field(info_ctx.ctx, nested.as_ptr()),
            Status::Err as libc::c_int
        );
    }

    #[test]
    fn rejects_new_section_while_dictionary_is_open() {
        let info_ctx = InfoContext::test();
        let first_section = CString::new("first").expect("section name should not contain NUL");
        let second_section = CString::new("second").expect("section name should not contain NUL");
        let dictionary =
            CString::new("dictionary").expect("dictionary name should not contain NUL");

        assert_eq!(
            info_add_section(info_ctx.ctx, first_section.as_ptr()),
            Status::Ok as libc::c_int
        );
        assert_eq!(
            info_begin_dict_field(info_ctx.ctx, dictionary.as_ptr()),
            Status::Ok as libc::c_int
        );
        assert_eq!(
            info_add_section(info_ctx.ctx, second_section.as_ptr()),
            Status::Err as libc::c_int
        );
        assert_eq!(info_ctx.sections().len(), 1);
    }

    #[test]
    fn rejects_null_field_name() {
        let info_ctx = InfoContext::test();

        assert_eq!(
            info_add_section(info_ctx.ctx, std::ptr::null()),
            Status::Ok as libc::c_int
        );
        assert_eq!(
            info_add_field_long_long(info_ctx.ctx, std::ptr::null(), 1),
            Status::Err as libc::c_int
        );
        assert!(info_ctx.sections()[0].entries.is_empty());
    }

    #[test]
    fn rejects_null_dictionary_name() {
        let info_ctx = InfoContext::test();

        assert_eq!(
            info_add_section(info_ctx.ctx, std::ptr::null()),
            Status::Ok as libc::c_int
        );
        assert_eq!(
            info_begin_dict_field(info_ctx.ctx, std::ptr::null()),
            Status::Err as libc::c_int
        );
        assert!(info_ctx.sections()[0].entries.is_empty());
    }

    #[test]
    fn rejects_null_string_value() {
        let info_ctx = InfoContext::test();
        let field = CString::new("field").expect("field name should not contain NUL");

        assert_eq!(
            info_add_section(info_ctx.ctx, std::ptr::null()),
            Status::Ok as libc::c_int
        );
        assert_eq!(
            info_add_field_string(info_ctx.ctx, field.as_ptr(), std::ptr::null_mut()),
            Status::Err as libc::c_int
        );
        assert!(info_ctx.sections()[0].entries.is_empty());
    }

    #[test]
    fn rejects_field_after_unrequested_section() {
        let mut info_ctx = InfoContext::test();
        info_ctx.expect_unrequested_section("hidden");
        let section = CString::new("hidden").expect("section name should not contain NUL");
        let field = CString::new("field").expect("field name should not contain NUL");

        assert_eq!(
            info_add_section(info_ctx.ctx, section.as_ptr()),
            Status::Err as libc::c_int
        );
        assert_eq!(
            info_add_field_long_long(info_ctx.ctx, field.as_ptr(), 1),
            Status::Err as libc::c_int
        );
        assert!(info_ctx.sections().is_empty());
    }

    #[test]
    fn accepts_scalar_field_after_dictionary_ends() {
        let info_ctx = InfoContext::test();
        let section = CString::new("section").expect("section name should not contain NUL");
        let dictionary =
            CString::new("dictionary").expect("dictionary name should not contain NUL");
        let field = CString::new("field").expect("field name should not contain NUL");

        assert_eq!(
            info_add_section(info_ctx.ctx, section.as_ptr()),
            Status::Ok as libc::c_int
        );
        assert_eq!(
            info_begin_dict_field(info_ctx.ctx, dictionary.as_ptr()),
            Status::Ok as libc::c_int
        );
        assert_eq!(info_end_dict_field(info_ctx.ctx), Status::Ok as libc::c_int);
        assert_eq!(
            info_add_field_long_long(info_ctx.ctx, field.as_ptr(), 1),
            Status::Ok as libc::c_int
        );
        assert_eq!(
            info_ctx.sections()[0].entries,
            vec![
                TestInfoEntry::Dictionary {
                    name: "dictionary".to_owned(),
                    fields: Vec::new(),
                },
                TestInfoEntry::Field(TestInfoField {
                    name: "field".to_owned(),
                    value: TestInfoValue::I64(1),
                }),
            ]
        );
    }

    #[test]
    fn rejects_invalid_dictionary_order() {
        let info = InfoContext::test();
        assert_eq!(info_end_dict_field(info.ctx), Status::Err as libc::c_int);
    }
}
