use crate::redismodule::{AUTH_HANDLED, AUTH_NOT_HANDLED};
use crate::{raw, Context, ValkeyError, ValkeyString};
use std::any::Any;
use std::os::raw::{c_int, c_void};
use std::ptr;

pub type AuthReplyCallback = fn(&Context, ValkeyString, ValkeyString) -> Result<c_int, ValkeyError>;
type FreePrivDataCallback<T> = fn(&Context, T);

static mut AUTH_REPLY_CALLBACK: Option<AuthReplyCallback> = None;
static mut FREE_PRIV_CALLBACK: Option<Box<dyn Fn(&Context, *mut c_void) + Send + Sync>> = None;

pub struct BlockedClient {
    pub(crate) inner: *mut raw::RedisModuleBlockedClient,
    private_data: Option<Box<dyn Any + Send>>,
}

pub unsafe extern "C" fn raw_free_callback_wrapper<T: 'static>(
    ctx: *mut raw::RedisModuleCtx,
    data: *mut c_void,
) {
    let ctx = &Context::new(ctx);

    if data.is_null() {
        ctx.log_debug("[callback] Data is null; this should not happen!");
        return;
    }

    let free_callback_ptr: *const Option<Box<dyn Fn(&Context, *mut c_void) + Send + Sync>> =
        &raw const FREE_PRIV_CALLBACK;
    if let Some(callback) = &*free_callback_ptr {
        callback(ctx, data);
    }
}

pub unsafe extern "C" fn reply_callback_wrapper(
    ctx: *mut raw::RedisModuleCtx,
    username: *mut raw::RedisModuleString,
    password: *mut raw::RedisModuleString,
    err: *mut *mut raw::RedisModuleString,
) -> c_int {
    let context = Context::new(ctx);
    let ctx_ptr = std::ptr::NonNull::new_unchecked(ctx);

    let username = ValkeyString::new(Some(ctx_ptr), username);
    let password = ValkeyString::new(Some(ctx_ptr), password);

    if let Some(callback) = AUTH_REPLY_CALLBACK {
        match callback(&context, username, password) {
            Ok(result) => result,
            Err(e) => {
                if !err.is_null() {
                    let error_msg = ValkeyString::create(None, e.to_string().as_str());
                    *err = error_msg.into_raw();
                }
                AUTH_HANDLED
            }
        }
    } else {
        AUTH_NOT_HANDLED
    }
}

// We need to be able to send the inner pointer to another thread
unsafe impl Send for BlockedClient {}

impl BlockedClient {
    pub(crate) fn new(inner: *mut raw::RedisModuleBlockedClient) -> Self {
        Self {
            inner,
            private_data: None,
        }
    }

    pub fn set_blocked_private_data<T: Any + Send>(&mut self, data: T) {
        self.private_data = Some(Box::new(data));
    }

    pub fn abort(mut self) -> Result<(), ValkeyError> {
        unsafe {
            // Clear private data first
            self.private_data = None;

            if raw::RedisModule_AbortBlock.unwrap()(self.inner) == raw::REDISMODULE_OK as c_int {
                // Prevent the normal Drop from running
                self.inner = std::ptr::null_mut();
                Ok(())
            } else {
                Err(ValkeyError::Str("Failed to abort blocked client"))
            }
        }
    }
}

impl Drop for BlockedClient {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            let privdata = self.private_data.take().map_or(ptr::null_mut(), |data| {
                Box::into_raw(data) as *mut std::ffi::c_void
            });

            unsafe {
                raw::RedisModule_UnblockClient.unwrap()(self.inner, privdata);
            };
        }
    }
}

impl Context {
    #[must_use]
    pub fn block_client(&self) -> BlockedClient {
        let blocked_client = unsafe {
            raw::RedisModule_BlockClient.unwrap()(
                self.ctx, // ctx
                None,     // reply_func
                None,     // timeout_func
                None, 0,
            )
        };

        BlockedClient::new(blocked_client)
    }

    #[must_use]
    pub fn block_client_on_auth<T: 'static>(
        &self,
        auth_reply_callback: AuthReplyCallback,
        free_privdata_callback: Option<FreePrivDataCallback<T>>,
    ) -> BlockedClient {
        unsafe {
            AUTH_REPLY_CALLBACK = Some(auth_reply_callback);

            let free_callback: Option<unsafe extern "C" fn(*mut raw::RedisModuleCtx, *mut c_void)> =
                if let Some(callback) = free_privdata_callback {
                    FREE_PRIV_CALLBACK = Some(Box::new(move |ctx: &Context, data: *mut c_void| {
                        let value = ptr::read(data as *const T);
                        callback(ctx, value);
                    }));
                    Some(raw_free_callback_wrapper::<T>)
                } else {
                    FREE_PRIV_CALLBACK = None;
                    None
                };

            let blocked_client = raw::RedisModule_BlockClientOnAuth.unwrap()(
                self.ctx,
                Some(reply_callback_wrapper),
                free_callback,
            );

            BlockedClient::new(blocked_client)
        }
    }
}
