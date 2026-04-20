use std::ffi::{CStr, CString};
use std::ptr::{self, NonNull};

use crate::Context;
use crate::{raw, ValkeyString};

pub struct ServerInfo {
    ctx: *mut raw::RedisModuleCtx,
    pub(crate) inner: *mut raw::RedisModuleServerInfoData,
}

impl Drop for ServerInfo {
    fn drop(&mut self) {
        unsafe { raw::RedisModule_FreeServerInfo.unwrap()(self.ctx, self.inner) };
    }
}

impl ServerInfo {
    /// Creates a new `ServerInfo` without requiring a context.
    ///
    /// The underlying Module API permits a NULL context for both
    /// `GetServerInfo` and `FreeServerInfo`.
    #[must_use]
    pub fn new(section: &str) -> Self {
        let section = CString::new(section).unwrap();
        let inner =
            unsafe { raw::RedisModule_GetServerInfo.unwrap()(ptr::null_mut(), section.as_ptr()) };
        Self {
            ctx: ptr::null_mut(),
            inner,
        }
    }

    /// Returns a field value as a `ValkeyString`.
    ///
    /// This works both with and without a context. When no context is
    /// available, the returned `ValkeyString` will not be registered with the
    /// auto memory mechanism, but Rust's `Drop` ensures proper cleanup.
    pub fn field(&self, field: &str) -> Option<ValkeyString> {
        let field = CString::new(field).unwrap();
        let value = unsafe {
            raw::RedisModule_ServerInfoGetField.unwrap()(self.ctx, self.inner, field.as_ptr())
        };
        if value.is_null() {
            None
        } else {
            Some(ValkeyString::new(NonNull::new(self.ctx), value))
        }
    }

    /// Returns a field value as a `&str`. Does not require a context.
    ///
    /// The returned string borrows from the `ServerInfo` data and is valid
    /// for the lifetime of this `ServerInfo`.
    pub fn field_c(&self, field: &str) -> Option<&str> {
        let field = CString::new(field).unwrap();
        let value =
            unsafe { raw::RedisModule_ServerInfoGetFieldC.unwrap()(self.inner, field.as_ptr()) };
        if value.is_null() {
            None
        } else {
            unsafe { CStr::from_ptr(value) }.to_str().ok()
        }
    }

    /// Returns a field value as a signed integer. Does not require a context.
    pub fn field_signed(&self, field: &str) -> Option<i64> {
        let field = CString::new(field).unwrap();
        let mut err: std::os::raw::c_int = 0;
        let value = unsafe {
            raw::RedisModule_ServerInfoGetFieldSigned.unwrap()(self.inner, field.as_ptr(), &mut err)
        };
        if err != 0 {
            None
        } else {
            Some(value)
        }
    }

    /// Returns a field value as an unsigned integer. Does not require a context.
    pub fn field_unsigned(&self, field: &str) -> Option<u64> {
        let field = CString::new(field).unwrap();
        let mut err: std::os::raw::c_int = 0;
        let value = unsafe {
            raw::RedisModule_ServerInfoGetFieldUnsigned.unwrap()(
                self.inner,
                field.as_ptr(),
                &mut err,
            )
        };
        if err != 0 {
            None
        } else {
            Some(value)
        }
    }

    /// Returns a field value as a double. Does not require a context.
    pub fn field_double(&self, field: &str) -> Option<f64> {
        let field = CString::new(field).unwrap();
        let mut err: std::os::raw::c_int = 0;
        let value = unsafe {
            raw::RedisModule_ServerInfoGetFieldDouble.unwrap()(self.inner, field.as_ptr(), &mut err)
        };
        if err != 0 {
            None
        } else {
            Some(value)
        }
    }
}

impl Context {
    #[must_use]
    pub fn server_info(&self, section: &str) -> ServerInfo {
        let section = CString::new(section).unwrap();
        let server_info = unsafe {
            raw::RedisModule_GetServerInfo.unwrap()(
                self.ctx,         // ctx
                section.as_ptr(), // section
            )
        };

        ServerInfo {
            ctx: self.ctx,
            inner: server_info,
        }
    }
}
