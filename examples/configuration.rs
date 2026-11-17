use std::sync::{
    atomic::{AtomicBool, AtomicI64},
    Mutex,
};

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{
    config_group,
    configuration::{ConfigurationContext, ConfigurationFlags},
    enum_configuration, valkey_module, ConfigurationValue, Context, ValkeyError, ValkeyGILGuard,
    ValkeyResult, ValkeyString, ValkeyValue,
};

enum_configuration! {
    #[derive(PartialEq)]
    enum EnumConfiguration {
        Val1 = 1,
        Val2 = 2,
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum FeatureMode {
    Safe,
    Fast,
}

lazy_static! {
    static ref NUM_OF_CONFIGURATION_CHANGES: ValkeyGILGuard<i64> = ValkeyGILGuard::default();
    static ref CONFIGURATION_I64: ValkeyGILGuard<i64> = ValkeyGILGuard::default();
    static ref CONFIGURATION_REJECT_I64: ValkeyGILGuard<i64> = ValkeyGILGuard::default();
    static ref CONFIGURATION_ATOMIC_I64: AtomicI64 = AtomicI64::new(1);
    static ref CONFIGURATION_VALKEY_STRING: ValkeyGILGuard<ValkeyString> =
        ValkeyGILGuard::new(ValkeyString::create(None, "default"));
    static ref CONFIGURATION_REJECT_VALKEY_STRING: ValkeyGILGuard<ValkeyString> =
        ValkeyGILGuard::new(ValkeyString::create(None, "default"));
    static ref CONFIGURATION_STRING: ValkeyGILGuard<String> = ValkeyGILGuard::new("default".into());
    static ref CONFIGURATION_MUTEX_STRING: Mutex<String> = Mutex::new("default".into());
    static ref CONFIGURATION_ATOMIC_BOOL: AtomicBool = AtomicBool::default();
    static ref CONFIGURATION_BOOL: ValkeyGILGuard<bool> = ValkeyGILGuard::default();
    static ref CONFIGURATION_REJECT_BOOL: ValkeyGILGuard<bool> = ValkeyGILGuard::default();
    static ref CONFIGURATION_ENUM: ValkeyGILGuard<EnumConfiguration> =
        ValkeyGILGuard::new(EnumConfiguration::Val1);
    static ref CONFIGURATION_REJECT_ENUM: ValkeyGILGuard<EnumConfiguration> =
        ValkeyGILGuard::new(EnumConfiguration::Val1);
    static ref CONFIGURATION_MUTEX_ENUM: Mutex<EnumConfiguration> =
        Mutex::new(EnumConfiguration::Val1);
    static ref FEATURE_CHANGE: ValkeyGILGuard<(i64, i64, i64)> = ValkeyGILGuard::new((10, 10, 0));
}

fn validate_feature(
    _ctx: &ConfigurationContext,
    _name: &str,
    proposed: &feature::Config,
) -> Result<(), ValkeyError> {
    if proposed.minimum > proposed.maximum {
        return Err(ValkeyError::Str("minimum must not exceed maximum"));
    }
    Ok(())
}

fn on_feature_changed(
    config_ctx: &ConfigurationContext,
    _name: &str,
    previous: &feature::Config,
    current: &feature::Config,
) {
    let mut change = FEATURE_CHANGE.lock(config_ctx);
    *change = (previous.minimum, current.minimum, change.2 + 1);
}

config_group! {
    pub feature {
        minimum: i64 = 10, range = 0..=100,
        maximum: i64 = 100, range = 0..=100,
        enabled: bool = false,
        #[serde(alias = "legacy_mode")]
        mode: FeatureMode = FeatureMode::Safe,
        label: Option<String> = Some("default".to_owned()),
    }

    validate: validate_feature,
    on_changed: on_feature_changed,
    flags: ConfigurationFlags::DEFAULT,
}

fn on_configuration_changed<G, T: ConfigurationValue<G>>(
    config_ctx: &ConfigurationContext,
    _name: &str,
    _val: &'static T,
) {
    let mut val = NUM_OF_CONFIGURATION_CHANGES.lock(config_ctx);
    *val += 1
}

// Custom on_set handlers to add validation and conditionally
// reject upon config change.
fn on_string_config_set<G, T: ConfigurationValue<ValkeyString>>(
    config_ctx: &ConfigurationContext,
    _name: &str,
    val: &'static T,
) -> Result<(), ValkeyError> {
    let v = val.get(config_ctx);
    if v.to_string_lossy().contains("rejectvalue") {
        return Err(ValkeyError::Str("Rejected from custom string validation."));
    }
    Ok(())
}
fn on_i64_config_set<G, T: ConfigurationValue<i64>>(
    config_ctx: &ConfigurationContext,
    _name: &str,
    val: &'static T,
) -> Result<(), ValkeyError> {
    let v = val.get(config_ctx);
    if v == 123 {
        return Err(ValkeyError::Str("Rejected from custom i64 validation."));
    }
    Ok(())
}
fn on_bool_config_set<G, T: ConfigurationValue<bool>>(
    config_ctx: &ConfigurationContext,
    _name: &str,
    val: &'static T,
) -> Result<(), ValkeyError> {
    let v = val.get(config_ctx);
    if v == false {
        return Err(ValkeyError::Str("Rejected from custom bool validation."));
    }
    Ok(())
}
fn on_enum_config_set<G, T: ConfigurationValue<EnumConfiguration>>(
    config_ctx: &ConfigurationContext,
    _name: &str,
    val: &'static T,
) -> Result<(), ValkeyError> {
    let v = val.get(config_ctx);
    if v == EnumConfiguration::Val2 {
        return Err(ValkeyError::Str("Rejected from custom enum validation."));
    }
    Ok(())
}

fn num_changes(ctx: &Context, _: Vec<ValkeyString>) -> ValkeyResult {
    let val = NUM_OF_CONFIGURATION_CHANGES.lock(ctx);
    Ok(ValkeyValue::Integer(*val))
}

fn feature_minimum(ctx: &Context, _: Vec<ValkeyString>) -> ValkeyResult {
    let config = feature::CONFIG.lock(ctx);
    Ok(ValkeyValue::Integer(config.minimum))
}

fn feature_change(ctx: &Context, _: Vec<ValkeyString>) -> ValkeyResult {
    let change = FEATURE_CHANGE.lock(ctx);
    Ok(ValkeyValue::Array(vec![
        ValkeyValue::Integer(change.0),
        ValkeyValue::Integer(change.1),
        ValkeyValue::Integer(change.2),
    ]))
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "configuration",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["configuration.num_changes", num_changes, "", 0, 0, 0],
        ["configuration.feature_minimum", feature_minimum, "", 0, 0, 0],
        ["configuration.feature_change", feature_change, "", 0, 0, 0],
    ],
    configurations: [
        group: [feature],
        i64: [
            ["i64", &*CONFIGURATION_I64, 10, 0, 1000, ConfigurationFlags::DEFAULT, Some(Box::new(on_configuration_changed))],
            ["reject_i64", &*CONFIGURATION_REJECT_I64, 10, 0, 1000, ConfigurationFlags::DEFAULT, Some(Box::new(on_configuration_changed)), Some(Box::new(on_i64_config_set::<ValkeyString, ValkeyGILGuard<i64>>))],
            ["atomic_i64", &*CONFIGURATION_ATOMIC_I64, 10, 0, 1000, ConfigurationFlags::DEFAULT, Some(Box::new(on_configuration_changed))],
        ],
        string: [
            ["valkey_string", &*CONFIGURATION_VALKEY_STRING, "default", ConfigurationFlags::DEFAULT, Some(Box::new(on_configuration_changed))],
            ["reject_valkey_string", &*CONFIGURATION_REJECT_VALKEY_STRING, "default", ConfigurationFlags::DEFAULT, Some(Box::new(on_configuration_changed)), Some(Box::new(on_string_config_set::<ValkeyString, ValkeyGILGuard<ValkeyString>>))],
            ["string", &*CONFIGURATION_STRING, "default", ConfigurationFlags::DEFAULT, Some(Box::new(on_configuration_changed::<String, _>))],
            ["mutex_string", &*CONFIGURATION_MUTEX_STRING, "default", ConfigurationFlags::DEFAULT, Some(Box::new(on_configuration_changed::<String, _>))],
        ],
        bool: [
            ["atomic_bool", &*CONFIGURATION_ATOMIC_BOOL, true, ConfigurationFlags::DEFAULT, Some(Box::new(on_configuration_changed))],
            ["bool", &*CONFIGURATION_BOOL, true, ConfigurationFlags::DEFAULT, Some(Box::new(on_configuration_changed))],
            ["reject_bool", &*CONFIGURATION_REJECT_BOOL, true, ConfigurationFlags::DEFAULT, Some(Box::new(on_configuration_changed)), Some(Box::new(on_bool_config_set::<ValkeyString, ValkeyGILGuard<bool>>))],
        ],
        enum: [
            ["enum", &*CONFIGURATION_ENUM, EnumConfiguration::Val1, ConfigurationFlags::DEFAULT, Some(Box::new(on_configuration_changed))],
            ["reject_enum", &*CONFIGURATION_REJECT_ENUM, EnumConfiguration::Val1, ConfigurationFlags::DEFAULT, Some(Box::new(on_configuration_changed)), Some(Box::new(on_enum_config_set::<ValkeyString, ValkeyGILGuard<EnumConfiguration>>))],
            ["enum_mutex", &*CONFIGURATION_MUTEX_ENUM, EnumConfiguration::Val1, ConfigurationFlags::DEFAULT, Some(Box::new(on_configuration_changed))],
        ],
        module_args_as_configuration: true,
    ]
}
