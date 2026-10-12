use super::{ConfigurationContext, ConfigurationFlags};
use crate::context::thread_safe::{ValkeyGILGuard, ValkeyGILGuardScope, ValkeyLockIndicator};
use crate::{raw, Context, ValkeyError, ValkeyString};
use serde::de::DeserializeOwned;
use serde::de::{self, MapAccess, Visitor};
use serde::Deserializer;
use serde::Serialize;
use serde_json::{Map, Value};
use std::fmt;
use std::ops::Deref;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

pub const MAX_CONFIGURATION_GROUP_JSON_BYTES: usize = 64 * 1024;

struct FlatObjectVisitor;

impl<'de> Visitor<'de> for FlatObjectVisitor {
    type Value = Map<String, Value>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a flat JSON object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!("duplicate field `{key}`")));
            }

            let value = map.next_value::<Value>()?;
            if matches!(value, Value::Array(_) | Value::Object(_)) {
                return Err(de::Error::custom(
                    "nested objects and arrays are not supported",
                ));
            }
            values.insert(key, value);
        }
        Ok(values)
    }
}

fn invalid_json(error: impl fmt::Display) -> ValkeyError {
    ValkeyError::String(format!("Invalid JSON: {error}"))
}

fn parse_flat_object(input: &[u8]) -> Result<Map<String, Value>, ValkeyError> {
    if input.len() > MAX_CONFIGURATION_GROUP_JSON_BYTES {
        return Err(ValkeyError::String(format!(
            "Invalid JSON: configuration groups are limited to {MAX_CONFIGURATION_GROUP_JSON_BYTES} bytes"
        )));
    }

    let mut deserializer = serde_json::Deserializer::from_slice(input);
    let object = deserializer
        .deserialize_map(FlatObjectVisitor)
        .map_err(invalid_json)?;
    deserializer.end().map_err(invalid_json)?;
    Ok(object)
}

fn merge_object_patch(base: Value, patch: Map<String, Value>) -> Result<Value, ValkeyError> {
    let Value::Object(mut base) = base else {
        return Err(ValkeyError::Str(
            "Invalid configuration group base: expected an object",
        ));
    };

    base.extend(patch);
    Ok(Value::Object(base))
}

pub type ConfigurationGroupValidator<T> =
    fn(&ConfigurationContext, &str, &T) -> Result<(), ValkeyError>;
pub type ConfigurationGroupChanged<T> = fn(&ConfigurationContext, &str, &T, &T);
#[doc(hidden)]
pub type ConfigurationGroupPatcher<T> = fn(T, Value) -> Result<T, ValkeyError>;

#[doc(hidden)]
#[derive(Default)]
pub enum ConfigurationGroupPatchField<T> {
    #[default]
    Missing,
    Value(T),
}

impl<'de, T> serde::Deserialize<'de> for ConfigurationGroupPatchField<T>
where
    T: serde::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Self::Value)
    }
}

pub struct ConfigurationGroup<T> {
    name: &'static str,
    default: fn() -> T,
    flags: ConfigurationFlags,
    validate: Option<ConfigurationGroupValidator<T>>,
    on_changed: Option<ConfigurationGroupChanged<T>>,
    patch: Option<ConfigurationGroupPatcher<T>>,
    value: OnceLock<ValkeyGILGuard<T>>,
    serialized: OnceLock<ValkeyGILGuard<ValkeyString>>,
    pending_change: OnceLock<ValkeyGILGuard<Option<(T, T)>>>,
    first_set_completed: AtomicBool,
}

pub struct ConfigurationGroupGuard<'ctx, 'group, T, G: ValkeyLockIndicator> {
    inner: ValkeyGILGuardScope<'ctx, 'group, T, G>,
}

impl<T, G: ValkeyLockIndicator> Deref for ConfigurationGroupGuard<'_, '_, T, G> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> ConfigurationGroup<T> {
    pub const fn new(
        name: &'static str,
        default: fn() -> T,
        flags: ConfigurationFlags,
        validate: Option<ConfigurationGroupValidator<T>>,
        on_changed: Option<ConfigurationGroupChanged<T>>,
    ) -> Self {
        Self {
            name,
            default,
            flags,
            validate,
            on_changed,
            patch: None,
            value: OnceLock::new(),
            serialized: OnceLock::new(),
            pending_change: OnceLock::new(),
            first_set_completed: AtomicBool::new(false),
        }
    }

    #[doc(hidden)]
    pub const fn new_with_patcher(
        name: &'static str,
        default: fn() -> T,
        flags: ConfigurationFlags,
        validate: Option<ConfigurationGroupValidator<T>>,
        on_changed: Option<ConfigurationGroupChanged<T>>,
        patch: ConfigurationGroupPatcher<T>,
    ) -> Self {
        Self {
            name,
            default,
            flags,
            validate,
            on_changed,
            patch: Some(patch),
            value: OnceLock::new(),
            serialized: OnceLock::new(),
            pending_change: OnceLock::new(),
            first_set_completed: AtomicBool::new(false),
        }
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub const fn flags(&self) -> &ConfigurationFlags {
        &self.flags
    }

    pub fn lock<'ctx, 'group, G: ValkeyLockIndicator>(
        &'group self,
        ctx: &'ctx G,
    ) -> ConfigurationGroupGuard<'ctx, 'group, T, G> {
        let value = self
            .value
            .get_or_init(|| ValkeyGILGuard::new((self.default)()));
        ConfigurationGroupGuard {
            inner: value.lock(ctx),
        }
    }

    fn validate_metadata(&self) -> Result<(), ValkeyError> {
        if self.name.is_empty()
            || !self
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(ValkeyError::String(format!(
                "Invalid configuration group name '{}': use only ASCII letters, numbers, '_' or '-'",
                self.name
            )));
        }

        let unsupported = ConfigurationFlags::MEMORY.bits() | ConfigurationFlags::BITFLAGS.bits();
        if self.flags.bits() & unsupported != 0 {
            return Err(ValkeyError::String(format!(
                "Configuration flags MEMORY and BITFLAGS are not supported for group '{}'",
                self.name
            )));
        }
        Ok(())
    }

    fn redact_error(&self, stage: &str, error: ValkeyError) -> ValkeyError {
        if self.flags.bits() & ConfigurationFlags::SENSITIVE.bits() != 0 {
            ValkeyError::String(format!(
                "Invalid sensitive configuration '{}': {stage}",
                self.name
            ))
        } else {
            error
        }
    }
}

fn serialize_flat_value<T: Serialize>(name: &str, value: &T) -> Result<Value, ValkeyError> {
    let serialized = serde_json::to_value(value).map_err(|error| {
        ValkeyError::String(format!(
            "Invalid configuration '{name}': could not serialize value: {error}"
        ))
    })?;
    let Value::Object(fields) = &serialized else {
        return Err(ValkeyError::String(format!(
            "Invalid configuration '{name}': expected an object"
        )));
    };
    if let Some((field, _)) = fields
        .iter()
        .find(|(_, value)| matches!(value, Value::Array(_) | Value::Object(_)))
    {
        return Err(ValkeyError::String(format!(
            "Invalid configuration '{name}' field '{field}': nested objects and arrays are not supported"
        )));
    }
    Ok(serialized)
}

impl<T> ConfigurationGroup<T>
where
    T: Clone + DeserializeOwned + Serialize + 'static,
{
    fn initialize(&self) -> Result<(), ValkeyError> {
        self.validate_metadata()?;
        let default = (self.default)();
        serialize_flat_value(self.name, &default)
            .map_err(|error| self.redact_error("default serialization failed", error))?;
        let configuration_ctx = ConfigurationContext::new();
        if let Some(validate) = self.validate {
            validate(&configuration_ctx, self.name, &default)
                .map_err(|error| self.redact_error("default validation failed", error))?;
        }
        self.value.get_or_init(|| ValkeyGILGuard::new(default));
        Ok(())
    }

    fn current_json<G: ValkeyLockIndicator>(&self, ctx: &G) -> Result<String, ValkeyError> {
        let value = self.lock(ctx);
        serialize_flat_value(self.name, &*value)
            .map_err(|error| self.redact_error("serialization failed", error))?;
        serde_json::to_string(&*value)
            .map_err(|error| {
                ValkeyError::String(format!(
                    "Invalid configuration '{}': could not serialize value: {error}",
                    self.name
                ))
            })
            .map_err(|error| self.redact_error("serialization failed", error))
    }

    fn initialize_serialized_cache(
        &self,
        configuration_ctx: &ConfigurationContext,
    ) -> Result<String, ValkeyError> {
        let json = self.current_json(configuration_ctx)?;
        self.serialized
            .get_or_init(|| ValkeyGILGuard::new(ValkeyString::create(None, json.as_bytes())));
        Ok(json)
    }

    fn apply_pending_change(&self, configuration_ctx: &ConfigurationContext) {
        let Some(on_changed) = self.on_changed else {
            return;
        };
        let pending = self
            .pending_change
            .get_or_init(|| ValkeyGILGuard::new(None));
        let change = {
            let mut pending = pending.lock(configuration_ctx);
            pending.take()
        };
        if let Some((previous, current)) = change {
            on_changed(configuration_ctx, self.name, &previous, &current);
        }
    }

    fn apply_json(
        &self,
        configuration_ctx: &ConfigurationContext,
        input: &[u8],
    ) -> Result<(), ValkeyError> {
        let patch = parse_flat_object(input)
            .map_err(|error| {
                ValkeyError::String(format!(
                    "Invalid JSON for configuration '{}': {error}",
                    self.name
                ))
            })
            .map_err(|error| self.redact_error("JSON parsing failed", error))?;
        let supplied_fields = patch.keys().cloned().collect::<Vec<_>>().join(", ");
        let first_set_completed = self.first_set_completed.load(Ordering::Relaxed);
        let base = if first_set_completed {
            self.lock(configuration_ctx).clone()
        } else {
            (self.default)()
        };
        let candidate = if let Some(apply_patch) = self.patch {
            apply_patch(base, Value::Object(patch)).map_err(|error| {
                ValkeyError::String(format!(
                    "Invalid configuration '{}' for field(s) [{}]: {error}",
                    self.name, supplied_fields
                ))
            })
        } else {
            let base = serialize_flat_value(self.name, &base)
                .map_err(|error| self.redact_error("serialization failed", error))?;
            merge_object_patch(base, patch).and_then(|candidate| {
                serde_json::from_value(candidate).map_err(|error| {
                    ValkeyError::String(format!(
                        "Invalid configuration '{}' for field(s) [{}]: {error}",
                        self.name, supplied_fields
                    ))
                })
            })
        }
        .map_err(|error| self.redact_error("deserialization failed", error))?;
        serialize_flat_value(self.name, &candidate)
            .map_err(|error| self.redact_error("serialization failed", error))?;
        if let Some(validate) = self.validate {
            validate(configuration_ctx, self.name, &candidate)
                .map_err(|error| self.redact_error("validation failed", error))?;
        }
        let candidate_json = serde_json::to_string(&candidate)
            .map_err(|error| {
                ValkeyError::String(format!(
                    "Invalid configuration '{}': could not serialize value: {error}",
                    self.name
                ))
            })
            .map_err(|error| self.redact_error("serialization failed", error))?;

        let previous = self.lock(configuration_ctx).clone();
        {
            let value = self
                .value
                .get_or_init(|| ValkeyGILGuard::new((self.default)()));
            let mut current = value.lock(configuration_ctx);
            *current = candidate.clone();
        }
        if let Some(serialized) = self.serialized.get() {
            let mut serialized = serialized.lock(configuration_ctx);
            *serialized = ValkeyString::create(None, candidate_json.as_bytes());
        }
        self.first_set_completed.store(true, Ordering::Relaxed);
        if let Some(on_changed) = self.on_changed {
            if first_set_completed {
                let pending = self
                    .pending_change
                    .get_or_init(|| ValkeyGILGuard::new(None));
                *pending.lock(configuration_ctx) = Some((previous, candidate));
            } else {
                on_changed(configuration_ctx, self.name, &previous, &candidate);
            }
        }
        Ok(())
    }

    pub fn register(&'static self, ctx: &Context) -> Result<(), ValkeyError> {
        self.initialize()?;
        let configuration_ctx = ConfigurationContext::new();
        let default_json = self.initialize_serialized_cache(&configuration_ctx)?;
        let name = std::ffi::CString::new(self.name).map_err(|_| {
            ValkeyError::String(format!(
                "Invalid configuration group name '{}': contains a NUL byte",
                self.name
            ))
        })?;
        let default_json = std::ffi::CString::new(default_json).map_err(|_| {
            ValkeyError::String(format!(
                "Invalid configuration '{}': serialized default contains a NUL byte",
                self.name
            ))
        })?;
        // SAFETY: Module API function pointers are initialized before RedisModule_OnLoad invokes
        // registration. Reading the optional pointer does not call it or mutate global state.
        let register = unsafe { raw::RedisModule_RegisterStringConfig }.ok_or(ValkeyError::Str(
            "String configuration registration API is unavailable",
        ))?;
        // SAFETY: `ctx` is the live module context passed to OnLoad. Both C strings live for the
        // duration of the call, the callbacks use the same `T`, and the private pointer refers to
        // this static configuration group for the process lifetime.
        let status = unsafe {
            register(
                ctx.ctx,
                name.as_ptr(),
                default_json.as_ptr(),
                self.flags.bits(),
                Some(configuration_group_get::<T>),
                Some(configuration_group_set::<T>),
                Some(configuration_group_apply::<T>),
                (self as *const Self).cast_mut().cast::<c_void>(),
            )
        };
        if status != raw::REDISMODULE_OK as c_int {
            return Err(ValkeyError::String(format!(
                "Failed to register configuration group '{}'",
                self.name
            )));
        }
        Ok(())
    }
}

extern "C" fn configuration_group_get<T>(
    _name: *const c_char,
    private_data: *mut c_void,
) -> *mut raw::RedisModuleString
where
    T: Clone + DeserializeOwned + Serialize + 'static,
{
    if private_data.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: Registration passed a pointer to this static `ConfigurationGroup<T>` for this
    // monomorphized callback, and the null check above passed.
    let group = unsafe { &*private_data.cast::<ConfigurationGroup<T>>() };
    let configuration_ctx = ConfigurationContext::new();
    let Some(serialized) = group.serialized.get() else {
        return ptr::null_mut();
    };
    let serialized = serialized.lock(&configuration_ctx);
    serialized.inner
}

extern "C" fn configuration_group_set<T>(
    _name: *const c_char,
    value: *mut raw::RedisModuleString,
    private_data: *mut c_void,
    error: *mut *mut raw::RedisModuleString,
) -> c_int
where
    T: Clone + DeserializeOwned + Serialize + 'static,
{
    if private_data.is_null() || value.is_null() {
        set_ffi_error(
            error,
            &ValkeyError::Str("Invalid configuration group callback data"),
        );
        return raw::REDISMODULE_ERR as c_int;
    }
    // SAFETY: Registration passed a pointer to this static `ConfigurationGroup<T>` for this
    // monomorphized callback, and the null check above passed.
    let group = unsafe { &*private_data.cast::<ConfigurationGroup<T>>() };
    let value = ValkeyString::new(None, value);
    let configuration_ctx = ConfigurationContext::new();
    match group.apply_json(&configuration_ctx, value.as_slice()) {
        Ok(()) => raw::REDISMODULE_OK as c_int,
        Err(configuration_error) => {
            set_ffi_error(error, &configuration_error);
            raw::REDISMODULE_ERR as c_int
        }
    }
}

extern "C" fn configuration_group_apply<T>(
    _ctx: *mut raw::RedisModuleCtx,
    private_data: *mut c_void,
    error: *mut *mut raw::RedisModuleString,
) -> c_int
where
    T: Clone + DeserializeOwned + Serialize + 'static,
{
    if private_data.is_null() {
        set_ffi_error(
            error,
            &ValkeyError::Str("Invalid configuration group callback data"),
        );
        return raw::REDISMODULE_ERR as c_int;
    }
    // SAFETY: Registration passed a pointer to this static `ConfigurationGroup<T>` for this
    // monomorphized callback, and the null check above passed.
    let group = unsafe { &*private_data.cast::<ConfigurationGroup<T>>() };
    group.apply_pending_change(&ConfigurationContext::new());
    raw::REDISMODULE_OK as c_int
}

fn set_ffi_error(error: *mut *mut raw::RedisModuleString, value: &ValkeyError) {
    if !error.is_null() {
        let error_value = ValkeyString::create(None, value.to_string());
        // SAFETY: The native setter contract supplies a writable error-output pointer when it is
        // non-null. `take` transfers the newly allocated module string to Valkey exactly once.
        unsafe {
            *error = error_value.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        configuration_group_get, merge_object_patch, parse_flat_object, ConfigurationGroup,
        ConfigurationGroupChanged, ConfigurationGroupValidator, MAX_CONFIGURATION_GROUP_JSON_BYTES,
    };
    use crate::configuration::{ConfigurationContext, ConfigurationFlags};
    use crate::ValkeyError;
    use serde::{Deserialize, Serialize};
    use serde_json::{json, Value};
    use std::sync::Mutex;

    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    #[serde(deny_unknown_fields)]
    struct TestConfig {
        minimum: i64,
        maximum: i64,
        enabled: bool,
        label: Option<String>,
    }

    fn test_default() -> TestConfig {
        TestConfig {
            minimum: 10,
            maximum: 100,
            enabled: false,
            label: Some("default".to_owned()),
        }
    }

    fn validate_test_config(
        _ctx: &ConfigurationContext,
        _name: &str,
        proposed: &TestConfig,
    ) -> Result<(), ValkeyError> {
        if proposed.minimum > proposed.maximum {
            return Err(ValkeyError::Str("minimum must not exceed maximum"));
        }
        Ok(())
    }

    static CHANGES: Mutex<Vec<(TestConfig, TestConfig)>> = Mutex::new(Vec::new());

    fn record_test_change(
        _ctx: &ConfigurationContext,
        _name: &str,
        previous: &TestConfig,
        current: &TestConfig,
    ) {
        CHANGES
            .lock()
            .expect("change recorder lock should be available")
            .push((previous.clone(), current.clone()));
    }

    fn test_group(
        flags: ConfigurationFlags,
        validate: Option<ConfigurationGroupValidator<TestConfig>>,
        on_changed: Option<ConfigurationGroupChanged<TestConfig>>,
    ) -> ConfigurationGroup<TestConfig> {
        ConfigurationGroup::new("feature", test_default, flags, validate, on_changed)
    }

    #[test]
    fn parses_flat_scalar_fields() {
        let parsed =
            parse_flat_object(br#"{"integer":10,"float":1.5,"enabled":true,"name":"safe"}"#)
                .expect("flat object should parse");

        assert_eq!(parsed["integer"], json!(10));
        assert_eq!(parsed["float"], json!(1.5));
        assert_eq!(parsed["enabled"], json!(true));
        assert_eq!(parsed["name"], json!("safe"));
    }

    #[test]
    fn accepts_explicit_null() {
        let parsed =
            parse_flat_object(br#"{"optional":null}"#).expect("null is a supported flat value");

        assert_eq!(parsed["optional"], Value::Null);
    }

    #[test]
    fn merges_patch_without_dropping_omitted_fields() {
        let base = json!({"minimum": 10, "maximum": 100, "enabled": false});
        let patch =
            parse_flat_object(br#"{"maximum":80,"enabled":true}"#).expect("patch should parse");

        let merged = merge_object_patch(base, patch).expect("objects should merge");

        assert_eq!(
            merged,
            json!({"minimum": 10, "maximum": 80, "enabled": true})
        );
    }

    #[test]
    fn rejects_malformed_json() {
        let error = parse_flat_object(br#"{"key":}"#).expect_err("malformed JSON must fail");

        assert!(error.to_string().contains("Invalid JSON"));
    }

    #[test]
    fn rejects_invalid_utf8() {
        let error = parse_flat_object(b"{\"key\":\"\xff\"}").expect_err("invalid UTF-8 must fail");

        assert!(error.to_string().contains("Invalid JSON"));
    }

    #[test]
    fn rejects_non_object_input() {
        let error = parse_flat_object(br#"[1,2,3]"#).expect_err("top level must be an object");

        assert!(error.to_string().contains("object"));
    }

    #[test]
    fn rejects_duplicate_keys() {
        let error =
            parse_flat_object(br#"{"key":1,"key":2}"#).expect_err("duplicate keys must fail");

        assert!(error.to_string().contains("duplicate field `key`"));
    }

    #[test]
    fn rejects_nested_objects() {
        let error = parse_flat_object(br#"{"nested":{"key":1}}"#)
            .expect_err("nested objects are not supported");

        assert!(error.to_string().contains("nested objects and arrays"));
    }

    #[test]
    fn rejects_nested_arrays() {
        let error =
            parse_flat_object(br#"{"nested":[1,2]}"#).expect_err("nested arrays are not supported");

        assert!(error.to_string().contains("nested objects and arrays"));
    }

    #[test]
    fn accepts_input_at_size_limit() {
        let wrapper_len = br#"{"key":""}"#.len();
        let padding = "a".repeat(MAX_CONFIGURATION_GROUP_JSON_BYTES - wrapper_len);
        let input = format!(r#"{{"key":"{padding}"}}"#);
        assert_eq!(input.len(), MAX_CONFIGURATION_GROUP_JSON_BYTES);

        let parsed = parse_flat_object(input.as_bytes()).expect("limit should be inclusive");

        assert_eq!(parsed["key"].as_str().map(str::len), Some(padding.len()));
    }

    #[test]
    fn rejects_input_over_size_limit() {
        let input = vec![b' '; MAX_CONFIGURATION_GROUP_JSON_BYTES + 1];

        let error = parse_flat_object(&input).expect_err("oversized input must fail first");

        assert!(error.to_string().contains("65536 bytes"));
    }

    #[test]
    fn rejects_non_object_merge_base() {
        let patch = parse_flat_object(br#"{"key":1}"#).expect("patch should parse");

        let error =
            merge_object_patch(json!("not an object"), patch).expect_err("base must be an object");

        assert!(error.to_string().contains("base"));
    }

    #[test]
    fn initializes_typed_storage_from_declared_default() {
        let group = test_group(ConfigurationFlags::DEFAULT, None, None);
        group
            .initialize()
            .expect("valid default should initialize storage");
        let ctx = ConfigurationContext::new();

        assert_eq!(*group.lock(&ctx), test_default());
    }

    #[test]
    fn applies_complete_typed_update() {
        let group = test_group(ConfigurationFlags::DEFAULT, None, None);
        group.initialize().expect("group should initialize");
        let ctx = ConfigurationContext::new();

        group
            .apply_json(
                &ctx,
                br#"{"minimum":20,"maximum":80,"enabled":true,"label":"live"}"#,
            )
            .expect("complete update should apply");

        assert_eq!(
            *group.lock(&ctx),
            TestConfig {
                minimum: 20,
                maximum: 80,
                enabled: true,
                label: Some("live".to_owned()),
            }
        );
    }

    #[test]
    fn applies_partial_updates_to_current_value() {
        let group = test_group(ConfigurationFlags::DEFAULT, None, None);
        group.initialize().expect("group should initialize");
        let ctx = ConfigurationContext::new();
        group
            .apply_json(&ctx, br#"{"minimum":20,"enabled":true}"#)
            .expect("first patch should apply");

        group
            .apply_json(&ctx, br#"{"maximum":60}"#)
            .expect("second patch should retain current fields");

        assert_eq!(group.lock(&ctx).minimum, 20);
        assert_eq!(group.lock(&ctx).maximum, 60);
        assert!(group.lock(&ctx).enabled);
    }

    #[test]
    fn applies_explicit_null_to_optional_field() {
        let group = test_group(ConfigurationFlags::DEFAULT, None, None);
        group.initialize().expect("group should initialize");
        let ctx = ConfigurationContext::new();

        group
            .apply_json(&ctx, br#"{"label":null}"#)
            .expect("optional field should accept null");

        assert_eq!(group.lock(&ctx).label, None);
    }

    #[test]
    fn serializes_complete_value_as_compact_json() {
        let group = test_group(ConfigurationFlags::DEFAULT, None, None);
        group.initialize().expect("group should initialize");
        let ctx = ConfigurationContext::new();

        assert_eq!(
            group.current_json(&ctx).expect("value should serialize"),
            r#"{"minimum":10,"maximum":100,"enabled":false,"label":"default"}"#
        );
    }

    #[test]
    fn getter_returns_the_same_module_owned_string() {
        let _ctx = crate::Context::test();
        let group = test_group(ConfigurationFlags::DEFAULT, None, None);
        group.initialize().expect("group should initialize");
        let configuration_ctx = ConfigurationContext::new();
        group
            .initialize_serialized_cache(&configuration_ctx)
            .expect("serialized cache should initialize");
        let private_data = (&group as *const ConfigurationGroup<TestConfig>)
            .cast_mut()
            .cast();

        let first = configuration_group_get::<TestConfig>(std::ptr::null(), private_data);
        let second = configuration_group_get::<TestConfig>(std::ptr::null(), private_data);

        assert!(!first.is_null());
        assert_eq!(first, second);
    }

    #[test]
    fn unknown_field_failure_preserves_current_value() {
        let group = test_group(ConfigurationFlags::DEFAULT, None, None);
        group.initialize().expect("group should initialize");
        let ctx = ConfigurationContext::new();
        let before = group.current_json(&ctx).expect("value should serialize");

        let error = group
            .apply_json(&ctx, br#"{"unknown":1}"#)
            .expect_err("unknown field must fail");

        assert!(error.to_string().contains("unknown field"));
        assert_eq!(
            group.current_json(&ctx).expect("value should serialize"),
            before
        );
    }

    #[test]
    fn type_failure_preserves_current_value() {
        let group = test_group(ConfigurationFlags::DEFAULT, None, None);
        group.initialize().expect("group should initialize");
        let ctx = ConfigurationContext::new();
        let before = group.current_json(&ctx).expect("value should serialize");

        let error = group
            .apply_json(&ctx, br#"{"minimum":"twenty"}"#)
            .expect_err("wrong field type must fail");

        assert!(error.to_string().contains("minimum"));
        assert_eq!(
            group.current_json(&ctx).expect("value should serialize"),
            before
        );
    }

    #[test]
    fn validator_failure_preserves_current_value() {
        let group = test_group(
            ConfigurationFlags::DEFAULT,
            Some(validate_test_config),
            None,
        );
        group.initialize().expect("group should initialize");
        let ctx = ConfigurationContext::new();
        let before = group.current_json(&ctx).expect("value should serialize");

        let error = group
            .apply_json(&ctx, br#"{"minimum":200}"#)
            .expect_err("cross-field validation must fail");

        assert!(error
            .to_string()
            .contains("minimum must not exceed maximum"));
        assert_eq!(
            group.current_json(&ctx).expect("value should serialize"),
            before
        );
    }

    #[test]
    fn changed_callback_receives_previous_and_published_values_once() {
        CHANGES
            .lock()
            .expect("change recorder lock should be available")
            .clear();
        let group = test_group(ConfigurationFlags::DEFAULT, None, Some(record_test_change));
        group.initialize().expect("group should initialize");
        let ctx = ConfigurationContext::new();

        group
            .apply_json(&ctx, br#"{"minimum":20}"#)
            .expect("update should apply");
        group
            .apply_json(&ctx, br#"{}"#)
            .expect("no-op patch should still notify");

        assert_eq!(
            CHANGES
                .lock()
                .expect("change recorder lock should be available")
                .len(),
            1,
            "runtime callbacks wait for the native apply phase"
        );
        group.apply_pending_change(&ctx);

        let changes = CHANGES
            .lock()
            .expect("change recorder lock should be available");
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].0.minimum, 10);
        assert_eq!(changes[0].1.minimum, 20);
        assert_eq!(changes[1].0, changes[1].1);
        assert_eq!(group.lock(&ctx).minimum, changes[1].1.minimum);
    }

    #[test]
    fn rejects_flags_that_only_apply_to_numeric_or_enum_configs() {
        for flags in [ConfigurationFlags::MEMORY, ConfigurationFlags::BITFLAGS] {
            let group = test_group(flags, None, None);

            let error = group
                .validate_metadata()
                .expect_err("unsupported group flag must fail");

            assert!(error.to_string().contains("not supported"));
        }
    }

    #[test]
    fn rejects_invalid_group_name() {
        let group = ConfigurationGroup::new(
            "feature.invalid",
            test_default,
            ConfigurationFlags::DEFAULT,
            None,
            None,
        );

        let error = group
            .validate_metadata()
            .expect_err("dot is reserved for the module separator");

        assert!(error.to_string().contains("name"));
    }

    #[derive(Clone, Deserialize, Serialize)]
    struct NestedDefault {
        nested: Vec<i64>,
    }

    fn nested_default() -> NestedDefault {
        NestedDefault { nested: vec![1] }
    }

    #[test]
    fn rejects_nested_default_before_registration() {
        let group = ConfigurationGroup::new(
            "nested",
            nested_default,
            ConfigurationFlags::DEFAULT,
            None,
            None,
        );

        let error = group
            .initialize()
            .expect_err("nested default must fail before native registration");

        assert!(error.to_string().contains("nested objects and arrays"));
    }

    #[derive(Clone, Deserialize, Serialize)]
    #[serde(rename_all = "lowercase")]
    enum SensitiveMode {
        Safe,
    }

    #[derive(Clone, Deserialize, Serialize)]
    struct SensitiveConfig {
        mode: SensitiveMode,
    }

    fn sensitive_default() -> SensitiveConfig {
        SensitiveConfig {
            mode: SensitiveMode::Safe,
        }
    }

    #[test]
    fn sensitive_group_errors_do_not_echo_field_values() {
        let group = ConfigurationGroup::new(
            "credentials",
            sensitive_default,
            ConfigurationFlags::SENSITIVE,
            None,
            None,
        );
        group.initialize().expect("group should initialize");
        let ctx = ConfigurationContext::new();

        let error = group
            .apply_json(&ctx, br#"{"mode":"secret-payload"}"#)
            .expect_err("invalid enum value must fail");

        assert!(!error.to_string().contains("secret-payload"));
    }
}
