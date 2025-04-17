use crate::{raw, Context, ValkeyString};
use std::os::raw::{c_char, c_int};
use std::ptr;

impl Context {
    /// Authenticates a client using an ACL user
    ///
    /// # Arguments
    /// * `username` - ACL username to authenticate with
    ///
    /// # Returns
    /// * `Status::Ok` - Authentication successful
    /// * `Status::Err` - Authentication failed
    pub fn authenticate_client_with_acl_user(&self, username: &ValkeyString) -> raw::Status {
        let result = unsafe {
            raw::RedisModule_AuthenticateClientWithACLUser.unwrap()(
                self.ctx,
                username.as_ptr().cast::<c_char>(),
                username.len(),
                None,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };

        if result == raw::REDISMODULE_OK as c_int {
            raw::Status::Ok
        } else {
            raw::Status::Err
        }
    }
}
