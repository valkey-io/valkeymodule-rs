use crate::redismodule::AUTH_HANDLED;
use crate::{raw, Context, ValkeyError, ValkeyString};
use std::os::raw::{c_int, c_void};

// Callback types for handling blocked client operations
// Currently supports authentication reply callback for block_client_on_auth
#[derive(Debug)]
pub enum ReplyCallback<T> {
    Auth(fn(&Context, ValkeyString, ValkeyString, Option<&T>) -> Result<c_int, ValkeyError>),
}

#[derive(Debug)]
struct BlockedClientPrivateData<T: 'static> {
    reply_callback: Option<ReplyCallback<T>>,
    free_callback: Option<FreePrivateDataCallback<T>>,
    data: Option<Box<T>>,
}

// Callback type for freeing private data associated with a blocked client
type FreePrivateDataCallback<T> = fn(&Context, T);

pub struct BlockedClient<T: 'static = ()> {
    pub(crate) inner: *mut raw::RedisModuleBlockedClient,
    reply_callback: Option<ReplyCallback<T>>,
    free_callback: Option<FreePrivateDataCallback<T>>,
    data: Option<Box<T>>,
}

#[allow(dead_code)]
unsafe extern "C" fn auth_reply_wrapper<T: 'static>(
    ctx: *mut raw::RedisModuleCtx,
    username: *mut raw::RedisModuleString,
    password: *mut raw::RedisModuleString,
    err: *mut *mut raw::RedisModuleString,
) -> c_int {
    let context = Context::new(ctx);
    let ctx_ptr = std::ptr::NonNull::new_unchecked(ctx);
    let username = ValkeyString::new(Some(ctx_ptr), username);
    let password = ValkeyString::new(Some(ctx_ptr), password);

    let module_private_data = context.get_blocked_client_private_data();
    if module_private_data.is_null() {
        panic!("[auth_reply_wrapper] Module private data is null; this should not happen!");
    }

    let user_private_data = &*(module_private_data as *const BlockedClientPrivateData<T>);

    let cb = match user_private_data.reply_callback.as_ref() {
        Some(ReplyCallback::Auth(cb)) => cb,
        None => panic!("[auth_reply_wrapper] Reply callback is null; this should not happen!"),
    };

    let data_ref = user_private_data.data.as_deref();

    match cb(&context, username, password, data_ref) {
        Ok(result) => result,
        Err(error) => {
            let error_msg = ValkeyString::create_and_retain(&error.to_string());
            *err = error_msg.inner;
            AUTH_HANDLED
        }
    }
}

#[allow(dead_code)]
unsafe extern "C" fn free_callback_wrapper<T: 'static>(
    ctx: *mut raw::RedisModuleCtx,
    module_private_data: *mut c_void,
) {
    let context = Context::new(ctx);

    if module_private_data.is_null() {
        panic!("[free_callback_wrapper] Module private data is null; this should not happen!");
    }

    let user_private_data = Box::from_raw(module_private_data as *mut BlockedClientPrivateData<T>);

    // Execute free_callback only if both callback and data exist
    // Note: free_callback can exist without data - this is a valid state
    if let Some(free_cb) = user_private_data.free_callback {
        if let Some(data) = user_private_data.data {
            free_cb(&context, *data);
        }
    }
}

// We need to be able to send the inner pointer to another thread
unsafe impl<T> Send for BlockedClient<T> {}

impl<T> BlockedClient<T> {
    pub(crate) fn new(inner: *mut raw::RedisModuleBlockedClient) -> Self {
        Self {
            inner,
            reply_callback: None,
            free_callback: None,
            data: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn with_auth_callback(
        inner: *mut raw::RedisModuleBlockedClient,
        auth_reply_callback: fn(
            &Context,
            ValkeyString,
            ValkeyString,
            Option<&T>,
        ) -> Result<c_int, ValkeyError>,
        free_callback: Option<FreePrivateDataCallback<T>>,
    ) -> Self
    where
        T: 'static,
    {
        Self {
            inner,
            reply_callback: Some(ReplyCallback::Auth(auth_reply_callback)),
            free_callback,
            data: None,
        }
    }

    /// Sets private data for the blocked client.
    ///
    /// # Panics
    /// This method will panic if called without first setting a free callback.
    /// The free callback is required to properly clean up any resources associated
    /// with the private data.
    ///
    /// # Arguments
    /// * `data` - The private data to store
    pub fn set_blocked_private_data(&mut self, data: T) {
        if self.free_callback.is_none() {
            panic!("Cannot set private data without a free callback - this would leak memory");
        }
        self.data = Some(Box::new(data));
    }

    /// Aborts the blocked client operation
    ///
    /// # Returns
    /// * `Ok(())` - If the blocked client was successfully aborted
    /// * `Err(ValkeyError)` - If the abort operation failed
    pub fn abort(mut self) -> Result<(), ValkeyError> {
        unsafe {
            // Clear references to data and callbacks
            self.data = None;
            self.reply_callback = None;
            self.free_callback = None;

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

impl<T: 'static> Drop for BlockedClient<T> {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            let callback_data_ptr = if self.reply_callback.is_some() || self.free_callback.is_some()
            {
                Box::into_raw(Box::new(BlockedClientPrivateData {
                    reply_callback: self.reply_callback.take(),
                    free_callback: self.free_callback.take(),
                    data: self.data.take(),
                })) as *mut c_void
            } else {
                std::ptr::null_mut()
            };

            unsafe {
                raw::RedisModule_UnblockClient.unwrap()(self.inner, callback_data_ptr);
            }
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

    /// Blocks a client during authentication and registers callbacks
    ///
    /// Wrapper around ValkeyModule_BlockClientOnAuth. Used for asynchronous authentication
    /// processing.
    ///
    /// # Arguments
    /// * `auth_reply_callback` - Callback executed when authentication completes
    /// * `free_callback` - Optional callback for cleaning up private data
    ///
    /// # Returns
    /// * `BlockedClient<T>` - Handle to manage the blocked client
    #[must_use]
    #[cfg(all(any(
        feature = "min-redis-compatibility-version-7-2",
        feature = "min-valkey-compatibility-version-8-0"
    ),))]
    pub fn block_client_on_auth<T: 'static + Send>(
        &self,
        auth_reply_callback: fn(
            &Context,
            ValkeyString,
            ValkeyString,
            Option<&T>,
        ) -> Result<c_int, ValkeyError>,
        free_callback: Option<FreePrivateDataCallback<T>>,
    ) -> BlockedClient<T> {
        unsafe {
            let blocked_client = raw::RedisModule_BlockClientOnAuth.unwrap()(
                self.ctx,
                Some(auth_reply_wrapper::<T>),
                Some(free_callback_wrapper::<T>),
            );

            BlockedClient::with_auth_callback(blocked_client, auth_reply_callback, free_callback)
        }
    }

    /// Retrieves the private data associated with a blocked client in the current context.
    /// This is an internal function used primarily by reply callbacks to access user-provided data.
    ///
    /// # Safety
    /// This function returns a raw pointer that must be properly cast to the expected type.
    /// The caller must ensure the pointer is not null before dereferencing.
    ///
    /// # Implementation Detail
    /// Wraps the Valkey Module C API function `ValkeyModule_GetBlockedClientPrivateData`
    pub(crate) fn get_blocked_client_private_data(&self) -> *mut c_void {
        unsafe { raw::RedisModule_GetBlockedClientPrivateData.unwrap()(self.ctx) }
    }
}
