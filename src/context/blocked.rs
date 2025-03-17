use crate::redismodule::{AUTH_HANDLED, AUTH_NOT_HANDLED};
use crate::{raw, Context, ValkeyError, ValkeyString};
use std::os::raw::{c_int, c_void};

type AuthReplyCallback<T> =
    fn(&Context, ValkeyString, ValkeyString, T) -> Result<c_int, ValkeyError>;
type FreePrivDataCallback<T> = fn(&Context, T);

pub struct BlockedClient<T = ()> {
    pub(crate) inner: *mut raw::RedisModuleBlockedClient,
    auth_reply_callback: Option<AuthReplyCallback<T>>,
    free_priv_callback: Option<FreePrivDataCallback<T>>,
    private_data: Option<Box<T>>,
}

#[allow(dead_code)]
unsafe extern "C" fn auth_reply_wrapper<T: 'static + Clone>(
    ctx: *mut raw::RedisModuleCtx,
    username: *mut raw::RedisModuleString,
    password: *mut raw::RedisModuleString,
    err: *mut *mut raw::RedisModuleString,
) -> c_int {
    let context = Context::new(ctx);
    let ctx_ptr = std::ptr::NonNull::new_unchecked(ctx);
    let username = ValkeyString::new(Some(ctx_ptr), username);
    let password = ValkeyString::new(Some(ctx_ptr), password);

    // Get private data from context
    let priv_data = context.get_blocked_client_privdata();
    if priv_data.is_null() {
        context.log_warning("[auth_reply_wrapper] Private data is null; this should not happen!");
        return AUTH_NOT_HANDLED;
    }

    let callbacks = &*(priv_data
        as *const (
            AuthReplyCallback<T>,
            Option<FreePrivDataCallback<T>>,
            Box<T>,
        ));

    match (callbacks.0)(&context, username, password, callbacks.2.as_ref().clone()) {
        Ok(status) => status,
        Err(error) => {
            let error_msg = ValkeyString::create(Some(ctx_ptr), error.to_string().as_str());
            *err = error_msg.into_raw();
            AUTH_HANDLED
        }
    }
}

#[allow(dead_code)]
unsafe extern "C" fn free_callback_wrapper<T: 'static>(
    ctx: *mut raw::RedisModuleCtx,
    privdata: *mut c_void,
) {
    let context = Context::new(ctx);

    if privdata.is_null() {
        context
            .log_warning("[free_callback_wrapper] Private data is null; this should not happen!");
        return;
    }

    let (_, free_cb, data) = *Box::from_raw(
        privdata
            as *mut (
                AuthReplyCallback<T>,
                Option<FreePrivDataCallback<T>>,
                Box<T>,
            ),
    );

    // free_cb is optional, so we only call free_cb if it exists
    // If no free_cb exists but the data(priv_data) exists, that's fine -
    // user might not allocate memory using VM_ALLOC or VM_REALLOC hence
    // it is not explicitly needed to free using VM_FREE and rust implicit
    // drop will handle cleanup. If memory was allocated using VM_ALLOC,
    // the module owners are responsible to provide free_privdata_callback
    // to avoid memory leaks.
    if let Some(free_cb) = free_cb {
        free_cb(&context, *data);
    }
}

// We need to be able to send the inner pointer to another thread
unsafe impl<T> Send for BlockedClient<T> {}

impl<T> BlockedClient<T> {
    pub(crate) fn new(inner: *mut raw::RedisModuleBlockedClient) -> Self {
        Self {
            inner,
            auth_reply_callback: None,
            free_priv_callback: None,
            private_data: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn with_auth_callback(
        inner: *mut raw::RedisModuleBlockedClient,
        auth_reply_callback: AuthReplyCallback<T>,
        free_priv_callback: Option<FreePrivDataCallback<T>>,
    ) -> Self {
        Self {
            inner,
            auth_reply_callback: Some(auth_reply_callback),
            free_priv_callback,
            private_data: None,
        }
    }

    pub fn set_blocked_private_data(&mut self, data: T) {
        self.private_data = Some(Box::new(data));
    }

    pub fn abort(mut self) -> Result<(), ValkeyError> {
        unsafe {
            // Clear references to private_data and callbacks
            self.private_data = None;
            self.auth_reply_callback = None;
            self.free_priv_callback = None;

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

impl<T> Drop for BlockedClient<T> {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            let auth_data = (
                self.auth_reply_callback,
                self.free_priv_callback,
                self.private_data.take(),
            );

            let auth_data_ptr = Box::into_raw(Box::new(auth_data)) as *mut c_void;
            unsafe {
                raw::RedisModule_UnblockClient.unwrap()(self.inner, auth_data_ptr);
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
    /// * `free_priv_callback` - Optional callback for cleaning up private data
    ///
    /// # Returns
    /// * `BlockedClient<T>` - Handle to manage the blocked client
    #[must_use]
    #[cfg(all(any(
        feature = "min-redis-compatibility-version-7-2",
        feature = "min-valkey-compatibility-version-8-0"
    ),))]
    pub fn block_client_on_auth<T: 'static + Send + Clone>(
        &self,
        auth_reply_callback: AuthReplyCallback<T>,
        free_priv_callback: Option<FreePrivDataCallback<T>>,
    ) -> BlockedClient<T> {
        unsafe {
            let blocked_client = raw::RedisModule_BlockClientOnAuth.unwrap()(
                self.ctx,
                Some(auth_reply_wrapper::<T>),
                Some(free_callback_wrapper::<T>),
            );

            BlockedClient::with_auth_callback(
                blocked_client,
                auth_reply_callback,
                free_priv_callback,
            )
        }
    }

    /// Retrieves private data associated with a blocked client
    ///
    /// Wrapper around ValkeyModule_GetBlockedClientPrivateData.
    #[cfg(all(any(
        feature = "min-redis-compatibility-version-7-0",
        feature = "min-valkey-compatibility-version-8-0"
    ),))]
    pub fn get_blocked_client_privdata(&self) -> *mut c_void {
        unsafe { raw::RedisModule_GetBlockedClientPrivateData.unwrap()(self.ctx) }
    }
}
