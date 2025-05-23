use crate::context::thread_safe::{ValkeyGILGuard, ValkeyLockIndicator};
use crate::{raw, CallOptionResp, CallOptionsBuilder, CallResult, ValkeyValue};
use crate::{Context, ValkeyError, ValkeyString};
use bitflags::bitflags;
use std::any::TypeId;
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::os::raw::{c_char, c_int, c_longlong, c_void};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Mutex;

bitflags! {
    /// Configuration options
    pub struct ConfigurationFlags : u32 {
        /// The default flags for a config. This creates a config that can be modified after startup.
        const DEFAULT = raw::REDISMODULE_CONFIG_DEFAULT;

        /// This config can only be provided loading time.
        const IMMUTABLE = raw::REDISMODULE_CONFIG_IMMUTABLE;

        /// The value stored in this config is redacted from all logging.
        const SENSITIVE = raw::REDISMODULE_CONFIG_SENSITIVE;

        /// The name is hidden from `CONFIG GET` with pattern matching.
        const HIDDEN = raw::REDISMODULE_CONFIG_HIDDEN;

        /// This config will be only be modifiable based off the value of enable-protected-configs.
        const PROTECTED = raw::REDISMODULE_CONFIG_PROTECTED;

        /// This config is not modifiable while the server is loading data.
        const DENY_LOADING = raw::REDISMODULE_CONFIG_DENY_LOADING;

        /// For numeric configs, this config will convert data unit notations into their byte equivalent.
        const MEMORY = raw::REDISMODULE_CONFIG_MEMORY;

        /// For enum configs, this config will allow multiple entries to be combined as bit flags.
        const BITFLAGS = raw::REDISMODULE_CONFIG_BITFLAGS;
    }
}

#[macro_export]
macro_rules! enum_configuration {
    ($(#[$meta:meta])* $vis:vis enum $name:ident {
        $($(#[$vmeta:meta])* $vname:ident = $val:expr,)*
    }) => {
        use $crate::configuration::EnumConfigurationValue;
        $(#[$meta])*
        $vis enum $name {
            $($(#[$vmeta])* $vname = $val,)*
        }

        impl std::convert::TryFrom<i32> for $name {
            type Error = $crate::ValkeyError;

            fn try_from(v: i32) -> Result<Self, Self::Error> {
                match v {
                    $(x if x == $name::$vname as i32 => Ok($name::$vname),)*
                    _ => Err($crate::ValkeyError::Str("Value is not supported")),
                }
            }
        }

        impl std::convert::From<$name> for i32 {
            fn from(val: $name) -> Self {
                val as i32
            }
        }

        impl EnumConfigurationValue for $name {
            fn get_options(&self) -> (Vec<String>, Vec<i32>) {
                (vec![$(stringify!($vname).to_string(),)*], vec![$($val,)*])
            }
        }

        impl Clone for $name {
            fn clone(&self) -> Self {
                match self {
                    $($name::$vname => $name::$vname,)*
                }
            }
        }
    }
}

/// [`ConfigurationContext`] is used as a special context that indicate that we are
/// running with the Valkey GIL is held but we should not perform all the regular
/// operation we can perfrom on the regular Context.
pub struct ConfigurationContext {
    _dummy: usize, // We set some none public vairable here so user will not be able to construct such object
}

impl ConfigurationContext {
    fn new() -> ConfigurationContext {
        ConfigurationContext { _dummy: 0 }
    }
}

unsafe impl ValkeyLockIndicator for ConfigurationContext {}

pub trait ConfigurationValue<T>: Sync + Send {
    fn get(&self, ctx: &ConfigurationContext) -> T;
    fn set(&self, ctx: &ConfigurationContext, val: T) -> Result<(), ValkeyError>;
}

pub trait EnumConfigurationValue: TryFrom<i32, Error = ValkeyError> + Into<i32> + Clone {
    fn get_options(&self) -> (Vec<String>, Vec<i32>);
}

impl<T: Clone> ConfigurationValue<T> for ValkeyGILGuard<T> {
    fn get(&self, ctx: &ConfigurationContext) -> T {
        let value = self.lock(ctx);
        value.clone()
    }
    fn set(&self, ctx: &ConfigurationContext, val: T) -> Result<(), ValkeyError> {
        let mut value = self.lock(ctx);
        *value = val;
        Ok(())
    }
}

impl<T: Clone + Send> ConfigurationValue<T> for Mutex<T> {
    fn get(&self, _ctx: &ConfigurationContext) -> T {
        let value = self.lock().unwrap();
        value.clone()
    }
    fn set(&self, _ctx: &ConfigurationContext, val: T) -> Result<(), ValkeyError> {
        let mut value = self.lock().unwrap();
        *value = val;
        Ok(())
    }
}

impl ConfigurationValue<i64> for AtomicI64 {
    fn get(&self, _ctx: &ConfigurationContext) -> i64 {
        self.load(Ordering::Relaxed)
    }
    fn set(&self, _ctx: &ConfigurationContext, val: i64) -> Result<(), ValkeyError> {
        self.store(val, Ordering::Relaxed);
        Ok(())
    }
}

impl ConfigurationValue<ValkeyString> for ValkeyGILGuard<String> {
    fn get(&self, ctx: &ConfigurationContext) -> ValkeyString {
        let value = self.lock(ctx);
        ValkeyString::create(None, value.as_str())
    }
    fn set(&self, ctx: &ConfigurationContext, val: ValkeyString) -> Result<(), ValkeyError> {
        let mut value = self.lock(ctx);
        *value = val.try_as_str()?.to_string();
        Ok(())
    }
}

impl ConfigurationValue<ValkeyString> for Mutex<String> {
    fn get(&self, _ctx: &ConfigurationContext) -> ValkeyString {
        let value = self.lock().unwrap();
        ValkeyString::create(None, value.as_str())
    }
    fn set(&self, _ctx: &ConfigurationContext, val: ValkeyString) -> Result<(), ValkeyError> {
        let mut value = self.lock().unwrap();
        *value = val.try_as_str()?.to_string();
        Ok(())
    }
}

impl ConfigurationValue<bool> for AtomicBool {
    fn get(&self, _ctx: &ConfigurationContext) -> bool {
        self.load(Ordering::Relaxed)
    }
    fn set(&self, _ctx: &ConfigurationContext, val: bool) -> Result<(), ValkeyError> {
        self.store(val, Ordering::Relaxed);
        Ok(())
    }
}

type OnUpdatedCallback<T> = Box<dyn Fn(&ConfigurationContext, &str, &'static T)>;

type OnSetCallback<T> =
    Box<dyn Fn(&ConfigurationContext, &str, &'static T) -> Result<(), ValkeyError>>;

struct ConfigrationPrivateData<G, T: ConfigurationValue<G> + 'static> {
    variable: &'static T,
    on_changed: Option<OnUpdatedCallback<T>>,
    on_set: Option<OnSetCallback<T>>,
    phantom: PhantomData<G>,
}

impl<G, T: ConfigurationValue<G> + 'static> ConfigrationPrivateData<G, T> {
    fn set_val(&self, name: *const c_char, val: G, err: *mut *mut raw::RedisModuleString) -> c_int {
        // we know the GIL is held so it is safe to use Context::dummy().
        let configuration_ctx = ConfigurationContext::new();
        if let Err(e) = self.variable.set(&configuration_ctx, val) {
            let error_msg = ValkeyString::create(None, e.to_string().as_str());
            unsafe { *err = error_msg.take() };
            return raw::REDISMODULE_ERR as i32;
        }
        let c_str_name = unsafe { CStr::from_ptr(name) };
        if let Some(v) = self.on_set.as_ref() {
            let result = v(
                &configuration_ctx,
                c_str_name.to_str().unwrap(),
                self.variable,
            );
            if let Err(e) = result {
                let error_msg = ValkeyString::create(None, e.to_string().as_str());
                unsafe { *err = error_msg.take() };
                return raw::REDISMODULE_ERR as i32;
            }
        }
        if let Some(v) = self.on_changed.as_ref() {
            v(
                &configuration_ctx,
                c_str_name.to_str().unwrap(),
                self.variable,
            );
        }
        raw::REDISMODULE_OK as i32
    }

    fn get_val(&self) -> G {
        self.variable.get(&ConfigurationContext::new())
    }
}

extern "C" fn i64_configuration_set<T: ConfigurationValue<i64> + 'static>(
    name: *const c_char,
    val: c_longlong,
    privdata: *mut c_void,
    err: *mut *mut raw::RedisModuleString,
) -> c_int {
    let private_data = unsafe { &*(privdata as *const ConfigrationPrivateData<i64, T>) };
    private_data.set_val(name, val, err)
}

extern "C" fn i64_configuration_get<T: ConfigurationValue<i64> + 'static>(
    _name: *const c_char,
    privdata: *mut c_void,
) -> c_longlong {
    let private_data = unsafe { &*(privdata as *const ConfigrationPrivateData<i64, T>) };
    private_data.get_val()
}

pub fn register_i64_configuration<T: ConfigurationValue<i64>>(
    ctx: &Context,
    name: &str,
    variable: &'static T,
    default: i64,
    min: i64,
    max: i64,
    flags: ConfigurationFlags,
    on_changed: Option<OnUpdatedCallback<T>>,
    on_set: Option<OnSetCallback<T>>,
) {
    let name = CString::new(name).unwrap();
    let config_private_data = ConfigrationPrivateData {
        variable,
        on_changed,
        on_set,
        phantom: PhantomData::<i64>,
    };
    unsafe {
        raw::RedisModule_RegisterNumericConfig.unwrap()(
            ctx.ctx,
            name.as_ptr(),
            default,
            flags.bits(),
            min,
            max,
            Some(i64_configuration_get::<T>),
            Some(i64_configuration_set::<T>),
            None,
            Box::into_raw(Box::new(config_private_data)) as *mut c_void,
        );
    }
}

fn find_config_value<'a>(args: &'a [ValkeyString], name: &str) -> Option<&'a ValkeyString> {
    args.iter()
        .skip_while(|item| !item.as_slice().eq(name.as_bytes()))
        .nth(1)
}

pub fn get_i64_default_config_value(
    args: &[ValkeyString],
    name: &str,
    default: i64,
) -> Result<i64, ValkeyError> {
    find_config_value(args, name).map_or(Ok(default), |arg| {
        arg.try_as_str()?
            .parse::<i64>()
            .map_err(|e| ValkeyError::String(e.to_string()))
    })
}

extern "C" fn string_configuration_set<T: ConfigurationValue<ValkeyString> + 'static>(
    name: *const c_char,
    val: *mut raw::RedisModuleString,
    privdata: *mut c_void,
    err: *mut *mut raw::RedisModuleString,
) -> c_int {
    let new_val = ValkeyString::new(None, val);
    let private_data = unsafe { &*(privdata as *const ConfigrationPrivateData<ValkeyString, T>) };
    private_data.set_val(name, new_val, err)
}

extern "C" fn string_configuration_get<T: ConfigurationValue<ValkeyString> + 'static>(
    _name: *const c_char,
    privdata: *mut c_void,
) -> *mut raw::RedisModuleString {
    match TypeId::of::<T>() {
        // For ValkeyString type we want to use the method 'safe_clone' in order to not cause a memory leak. Due to this we will do the flow as before explicitly in this match
        // but instead of doing a clone at the end we do a 'safe_clone'
        valkeystringtype if valkeystringtype == TypeId::of::<ValkeyGILGuard<ValkeyString>>() => {
            let private_data = unsafe {
                &*(privdata
                    as *const ConfigrationPrivateData<ValkeyString, ValkeyGILGuard<ValkeyString>>)
            };
            let valkey_gilguard = private_data.variable;
            let ctx = &ConfigurationContext::new();
            let value = valkey_gilguard.lock(ctx);
            value.safe_clone(&Context::dummy()).inner
        }
        _ => {
            // For non ValkeyString types we can go through the typical flow of getting the config
            let private_data =
                unsafe { &*(privdata as *const ConfigrationPrivateData<ValkeyString, T>) };
            private_data
                .variable
                .get(&ConfigurationContext::new())
                .take()
        }
    }
}

pub fn register_string_configuration<T: ConfigurationValue<ValkeyString>>(
    ctx: &Context,
    name: &str,
    variable: &'static T,
    default: &str,
    flags: ConfigurationFlags,
    on_changed: Option<OnUpdatedCallback<T>>,
    on_set: Option<OnSetCallback<T>>,
) {
    let name = CString::new(name).unwrap();
    let default = CString::new(default).unwrap();
    let config_private_data = ConfigrationPrivateData {
        variable,
        on_changed,
        on_set,
        phantom: PhantomData::<ValkeyString>,
    };
    unsafe {
        raw::RedisModule_RegisterStringConfig.unwrap()(
            ctx.ctx,
            name.as_ptr(),
            default.as_ptr(),
            flags.bits(),
            Some(string_configuration_get::<T>),
            Some(string_configuration_set::<T>),
            None,
            Box::into_raw(Box::new(config_private_data)) as *mut c_void,
        );
    }
}

pub fn get_string_default_config_value<'a>(
    args: &'a [ValkeyString],
    name: &str,
    default: &'a str,
) -> Result<&'a str, ValkeyError> {
    find_config_value(args, name).map_or(Ok(default), |arg| arg.try_as_str())
}

extern "C" fn bool_configuration_set<T: ConfigurationValue<bool> + 'static>(
    name: *const c_char,
    val: i32,
    privdata: *mut c_void,
    err: *mut *mut raw::RedisModuleString,
) -> c_int {
    let private_data = unsafe { &*(privdata as *const ConfigrationPrivateData<bool, T>) };
    private_data.set_val(name, val != 0, err)
}

extern "C" fn bool_configuration_get<T: ConfigurationValue<bool> + 'static>(
    _name: *const c_char,
    privdata: *mut c_void,
) -> c_int {
    let private_data = unsafe { &*(privdata as *const ConfigrationPrivateData<bool, T>) };
    private_data.get_val() as i32
}

pub fn register_bool_configuration<T: ConfigurationValue<bool>>(
    ctx: &Context,
    name: &str,
    variable: &'static T,
    default: bool,
    flags: ConfigurationFlags,
    on_changed: Option<OnUpdatedCallback<T>>,
    on_set: Option<OnSetCallback<T>>,
) {
    let name = CString::new(name).unwrap();
    let config_private_data = ConfigrationPrivateData {
        variable,
        on_changed,
        on_set,
        phantom: PhantomData::<bool>,
    };
    unsafe {
        raw::RedisModule_RegisterBoolConfig.unwrap()(
            ctx.ctx,
            name.as_ptr(),
            default as i32,
            flags.bits(),
            Some(bool_configuration_get::<T>),
            Some(bool_configuration_set::<T>),
            None,
            Box::into_raw(Box::new(config_private_data)) as *mut c_void,
        );
    }
}

pub fn get_bool_default_config_value(
    args: &[ValkeyString],
    name: &str,
    default: bool,
) -> Result<bool, ValkeyError> {
    find_config_value(args, name).map_or(Ok(default), |arg| Ok(arg.try_as_str()? == "yes"))
}

extern "C" fn enum_configuration_set<
    G: EnumConfigurationValue,
    T: ConfigurationValue<G> + 'static,
>(
    name: *const c_char,
    val: i32,
    privdata: *mut c_void,
    err: *mut *mut raw::RedisModuleString,
) -> c_int {
    let private_data = unsafe { &*(privdata as *const ConfigrationPrivateData<G, T>) };
    let val: Result<G, _> = val.try_into();
    match val {
        Ok(val) => private_data.set_val(name, val, err),
        Err(e) => {
            let error_msg = ValkeyString::create(None, e.to_string().as_str());
            unsafe { *err = error_msg.take() };
            raw::REDISMODULE_ERR as i32
        }
    }
}

extern "C" fn enum_configuration_get<
    G: EnumConfigurationValue,
    T: ConfigurationValue<G> + 'static,
>(
    _name: *const c_char,
    privdata: *mut c_void,
) -> c_int {
    let private_data = unsafe { &*(privdata as *const ConfigrationPrivateData<G, T>) };
    private_data.get_val().into()
}

pub fn register_enum_configuration<G: EnumConfigurationValue, T: ConfigurationValue<G>>(
    ctx: &Context,
    name: &str,
    variable: &'static T,
    default: G,
    flags: ConfigurationFlags,
    on_changed: Option<OnUpdatedCallback<T>>,
    on_set: Option<OnSetCallback<T>>,
) {
    let name = CString::new(name).unwrap();
    let (names, vals) = default.get_options();
    assert_eq!(names.len(), vals.len());
    let names: Vec<CString> = names
        .into_iter()
        .map(|v| CString::new(v).unwrap())
        .collect();
    let config_private_data = ConfigrationPrivateData {
        variable,
        on_changed,
        on_set,
        phantom: PhantomData::<G>,
    };
    unsafe {
        raw::RedisModule_RegisterEnumConfig.unwrap()(
            ctx.ctx,
            name.as_ptr(),
            default.into(),
            flags.bits(),
            names
                .iter()
                .map(|v| v.as_ptr())
                .collect::<Vec<*const c_char>>()
                .as_mut_ptr(),
            vals.as_ptr(),
            names.len() as i32,
            Some(enum_configuration_get::<G, T>),
            Some(enum_configuration_set::<G, T>),
            None,
            Box::into_raw(Box::new(config_private_data)) as *mut c_void,
        );
    }
}

pub fn get_enum_default_config_value<G: EnumConfigurationValue>(
    args: &[ValkeyString],
    name: &str,
    default: G,
) -> Result<G, ValkeyError> {
    find_config_value(args, name).map_or(Ok(default.clone()), |arg| {
        let (names, vals) = default.get_options();
        let (index, _name) = names
            .into_iter()
            .enumerate()
            .find(|(_index, item)| item.as_bytes().eq(arg.as_slice()))
            .ok_or(ValkeyError::String(format!(
                "Enum '{}' not exists",
                arg.to_string_lossy()
            )))?;
        G::try_from(vals[index])
    })
}

pub fn module_config_get(
    ctx: &Context,
    args: Vec<ValkeyString>,
    name: &str,
) -> Result<ValkeyValue, ValkeyError> {
    let mut args: Vec<String> = args
        .into_iter()
        .skip(1)
        .map(|e| format!("{}.{}", name, e.to_string_lossy()))
        .collect();
    args.insert(0, "get".into());
    let res: CallResult = ctx.call_ext(
        "config",
        &CallOptionsBuilder::new()
            .errors_as_replies()
            .resp(CallOptionResp::Auto)
            .build(),
        args.iter()
            .map(|v| v.as_str())
            .collect::<Vec<&str>>()
            .as_slice(),
    );
    let res = res.map_err(|e| {
        ValkeyError::String(
            e.to_utf8_string()
                .unwrap_or("Failed converting error to utf8".into()),
        )
    })?;
    Ok((&res).into())
}

pub fn module_config_set(
    ctx: &Context,
    args: Vec<ValkeyString>,
    name: &str,
) -> Result<ValkeyValue, ValkeyError> {
    let mut args: Vec<String> = args
        .into_iter()
        .skip(1)
        .enumerate()
        .map(|(index, e)| {
            if index % 2 == 0 {
                format!("{}.{}", name, e.to_string_lossy())
            } else {
                e.to_string_lossy()
            }
        })
        .collect();
    args.insert(0, "set".into());
    let res: CallResult = ctx.call_ext(
        "config",
        &CallOptionsBuilder::new()
            .errors_as_replies()
            .resp(CallOptionResp::Auto)
            .build(),
        args.iter()
            .map(|v| v.as_str())
            .collect::<Vec<&str>>()
            .as_slice(),
    );
    let res = res.map_err(|e| {
        ValkeyError::String(
            e.to_utf8_string()
                .unwrap_or("Failed converting error to utf8".into()),
        )
    })?;
    Ok((&res).into())
}
