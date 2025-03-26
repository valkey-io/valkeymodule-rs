use crate::{
    Context, RedisModuleCommandFilter, RedisModuleCommandFilterCtx, RedisModuleString,
    RedisModule_CommandFilterArgDelete, RedisModule_CommandFilterArgGet,
    RedisModule_CommandFilterArgInsert, RedisModule_CommandFilterArgReplace,
    RedisModule_CommandFilterArgsCount, RedisModule_CommandFilterGetClientId,
    RedisModule_RegisterCommandFilter, RedisModule_UnregisterCommandFilter, ValkeyString,
};
use std::ffi::c_int;
use std::str::Utf8Error;

#[derive(Debug, Clone, Copy)]
pub struct CommandFilter {
    inner: *mut RedisModuleCommandFilter,
}

// otherwise you get error:     cannot be shared between threads safely
unsafe impl Send for CommandFilter {}
unsafe impl Sync for CommandFilter {}

/// CommandFilter is a wrapper around the RedisModuleCommandFilter pointer
///
/// It provides a way to check if the filter is null and to create a new CommandFilter
impl CommandFilter {
    pub fn new(inner: *mut RedisModuleCommandFilter) -> Self {
        CommandFilter { inner }
    }

    pub fn is_null(&self) -> bool {
        self.inner.is_null()
    }
}

pub struct CommandFilterCtx {
    inner: *mut RedisModuleCommandFilterCtx,
}

/// wrapping the RedisModuleCommandFilterCtx to provide a higher level interface to call RedisModule_CommandFilter*
///
/// provides functions to interact with the command filter context, such as getting the number of arguments, getting and replacing arguments, and deleting arguments.
impl CommandFilterCtx {
    pub fn new(inner: *mut RedisModuleCommandFilterCtx) -> Self {
        CommandFilterCtx { inner }
    }

    /// wrapper for RedisModule_CommandFilterArgsCount
    pub fn args_count(&self) -> c_int {
        unsafe { RedisModule_CommandFilterArgsCount.unwrap()(self.inner) }
    }

    /// wrapper for RedisModule_CommandFilterArgGet
    pub fn arg_get(&self, pos: c_int) -> *mut RedisModuleString {
        unsafe { RedisModule_CommandFilterArgGet.unwrap()(self.inner, pos) }
    }

    /// wrapper to get argument as a Result<&str, Utf8Error> instead of RedisModuleString
    pub fn arg_get_try_as_str<'a>(&self, pos: c_int) -> Result<&'a str, Utf8Error> {
        let arg = self.arg_get(pos);
        ValkeyString::from_ptr(arg)
    }

    /// wrapper to get 0 argument, the command which is always present and return as &str
    pub fn cmd_get_try_as_str<'a>(&self) -> Result<&'a str, Utf8Error> {
        let cmd = self.arg_get(0);
        ValkeyString::from_ptr(cmd)
    }

    /// wrapper to get Vector of all args minus the command (0th arg)
    pub fn get_all_args_wo_cmd(&self) -> Vec<&str> {
        let mut output = Vec::new();
        for pos in 1..self.args_count() {
            match self.arg_get_try_as_str(pos) {
                Ok(arg) => output.push(arg),
                Err(_) => continue, // skip invalid args
            }
        }
        output
    }

    /// wrapper for RedisModule_CommandFilterArgReplace, accepts simple &str and casts it to *mut RedisModuleString
    pub fn arg_replace(&self, pos: c_int, arg: &str) {
        unsafe {
            RedisModule_CommandFilterArgReplace.unwrap()(
                self.inner,
                pos,
                ValkeyString::create_and_retain(arg).inner,
            )
        };
    }

    /// wrapper for RedisModule_CommandFilterArgInsert, accepts simple &str and casts it to *mut RedisModuleString
    pub fn arg_insert(&self, pos: c_int, arg: &str) {
        unsafe {
            RedisModule_CommandFilterArgInsert.unwrap()(
                self.inner,
                pos,
                ValkeyString::create_and_retain(arg).inner,
            )
        };
    }

    /// wrapper for RedisModule_CommandFilterArgDelete
    pub fn arg_delete(&self, pos: c_int) {
        unsafe { RedisModule_CommandFilterArgDelete.unwrap()(self.inner, pos) };
    }

    /// wrapper for RedisModule_CommandFilterGetClientId, not supported in Redis 7.0
    #[cfg(all(any(
        feature = "min-redis-compatibility-version-7-2",
        feature = "min-valkey-compatibility-version-8-0"
    ),))]
    pub fn get_client_id(&self) -> u64 {
        unsafe { RedisModule_CommandFilterGetClientId.unwrap()(self.inner) }
    }
}

/// adding functions to the Context struct to provide a higher level interface to register and unregister filters
impl Context {
    /// wrapper for RedisModule_RegisterCommandFilter to directly register a filter, likely in init
    pub fn register_command_filter(
        &self,
        module_cmd_filter_func: extern "C" fn(*mut RedisModuleCommandFilterCtx),
        flags: u32,
    ) -> CommandFilter {
        let module_cmd_filter = unsafe {
            RedisModule_RegisterCommandFilter.unwrap()(
                self.ctx,
                Some(module_cmd_filter_func),
                flags as c_int,
            )
        };
        CommandFilter::new(module_cmd_filter)
    }

    /// wrapper for RedisModule_UnregisterCommandFilter to directly unregister filter, likely in deinit
    pub fn unregister_command_filter(&self, cmd_filter: &CommandFilter) {
        unsafe {
            RedisModule_UnregisterCommandFilter.unwrap()(self.ctx, cmd_filter.inner);
        }
    }
}
