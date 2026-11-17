use serde::{Deserialize, Serialize};
use valkey_module::configuration::{ConfigurationContext, ConfigurationFlags};
use valkey_module::{config_group, Context, ValkeyError};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum FeatureMode {
    Safe,
    Fast,
}

fn validate_feature(
    _ctx: &ConfigurationContext,
    _name: &str,
    proposed: &feature::Config,
) -> Result<(), ValkeyError> {
    if proposed.key2 && proposed.mode == FeatureMode::Safe {
        return Err(ValkeyError::Str("key2 requires fast mode"));
    }
    Ok(())
}

fn on_feature_changed(
    _ctx: &ConfigurationContext,
    _name: &str,
    _previous: &feature::Config,
    _current: &feature::Config,
) {
}

config_group! {
    pub feature {
        key1: i64 = 10, range = 0..=100,
        key2: bool = false,
        #[serde(rename = "feature-mode", alias = "mode")]
        mode: FeatureMode = FeatureMode::Safe,
    }

    validate: validate_feature,
    on_changed: on_feature_changed,
    flags: ConfigurationFlags::DEFAULT,
}

config_group! {
    pub defaults_only {
        enabled: bool = true,
    }
}

mod limits {
    pub const MIN: i64 = -20;
    pub const MAX: i64 = 20;
}

config_group! {
    pub expression_ranges {
        signed: i64 = 0, range = -10..=limits::MAX,
        qualified: i64 = 0, range = limits::MIN..=limits::MAX,
    }
}

#[test]
fn generates_typed_defaults_and_metadata() {
    let config = feature::Config::default();

    assert_eq!(config.key1, 10);
    assert!(!config.key2);
    assert_eq!(config.mode, FeatureMode::Safe);
    assert_eq!(feature::CONFIG.name(), "feature");
    assert_eq!(
        feature::CONFIG.flags().bits(),
        ConfigurationFlags::DEFAULT.bits()
    );
    assert_eq!(
        defaults_only::CONFIG.flags().bits(),
        ConfigurationFlags::DEFAULT.bits()
    );
}

#[test]
fn accepts_signed_and_qualified_range_bounds() {
    let proposed = expression_ranges::Config::default();
    assert_eq!(proposed.signed, 0);
    assert_eq!(proposed.qualified, 0);
}

#[test]
fn forwards_serde_field_attributes_and_rejects_unknown_fields() {
    let serialized = serde_json::to_value(feature::Config::default())
        .expect("generated config should serialize");

    assert_eq!(serialized["feature-mode"], serde_json::json!("safe"));
    assert!(serialized.get("mode").is_none());

    let from_alias: feature::Config = serde_json::from_value(serde_json::json!({
        "key1": 20,
        "key2": false,
        "mode": "fast"
    }))
    .expect("serde alias should be forwarded");
    assert_eq!(from_alias.mode, FeatureMode::Fast);

    let result = serde_json::from_value::<feature::Config>(serde_json::json!({
        "key1": 20,
        "key2": false,
        "feature-mode": "safe",
        "typo": true
    }));
    let error = match result {
        Ok(_) => panic!("generated config must deny unknown fields"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn exposes_read_only_typed_storage() {
    let ctx = Context::dummy();
    let config = feature::CONFIG.lock(&ctx);

    assert_eq!(config.key1, 10);
    assert_eq!(config.mode, FeatureMode::Safe);
}

#[test]
fn compile_fail_declarations_have_focused_diagnostics() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/config_group/*.rs");
    tests.pass("tests/ui/config_group-pass/*.rs");
}

#[allow(dead_code)]
mod registration_compile {
    use super::feature;
    use lazy_static::lazy_static;
    use std::sync::atomic::{AtomicBool, AtomicI64};
    use std::sync::Mutex;
    use valkey_module::configuration::ConfigurationFlags;
    use valkey_module::{enum_configuration, valkey_module};

    enum_configuration! {
        enum TestEnum {
            One = 1,
            Two = 2,
        }
    }

    static TEST_I64: AtomicI64 = AtomicI64::new(10);
    static TEST_BOOL: AtomicBool = AtomicBool::new(true);

    lazy_static! {
        static ref TEST_STRING: Mutex<String> = Mutex::new("default".to_owned());
        static ref TEST_ENUM: Mutex<TestEnum> = Mutex::new(TestEnum::One);
    }

    valkey_module! {
        name: "config_group_compile",
        version: 1,
        allocator: (std::alloc::System, std::alloc::System),
        data_types: [],
        commands: [],
        configurations: [
            group: [feature],
            i64: [
                ["number", &TEST_I64, 10, 0, 100, ConfigurationFlags::DEFAULT, None],
            ],
            string: [
                ["text", &*TEST_STRING, "default", ConfigurationFlags::DEFAULT, None],
            ],
            bool: [
                ["enabled", &TEST_BOOL, true, ConfigurationFlags::DEFAULT, None],
            ],
            enum: [
                ["choice", &*TEST_ENUM, TestEnum::One, ConfigurationFlags::DEFAULT, None],
            ],
            module_args_as_configuration: true,
        ]
    }
}
