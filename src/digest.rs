use crate::{raw, ValkeyString};

use std::os::raw::{c_char, c_int, c_longlong};

/// `Digest` is a high-level rust interface to the Valkey module C API
/// abstracting away the raw C ffi calls.
pub struct Digest {
    pub dig: *mut raw::RedisModuleDigest,
}

impl Digest {
    pub const fn new(dig: *mut raw::RedisModuleDigest) -> Self {
        Self { dig }
    }

    /// Returns the key name of this [`Digest`].
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_GetKeyNameFromDigest` is missing in redismodule.h
    pub fn get_key_name(&self) -> ValkeyString {
        ValkeyString::new(None, unsafe {
            raw::RedisModule_GetKeyNameFromDigest
                .expect("RedisModule_GetKeyNameFromDigest is not available.")(self.dig)
            .cast_mut()
        })
    }

    /// Returns the database ID of this [`Digest`].
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_GetDbIdFromDigest` is missing in redismodule.h
    pub fn get_db_id(&self) -> c_int {
        unsafe {
            raw::RedisModule_GetDbIdFromDigest
                .expect("RedisModule_GetDbIdFromDigest is not available.")(self.dig)
        }
    }

    /// Adds a new element to this [`Digest`].
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_DigestAddStringBuffer` is missing in redismodule.h
    pub fn add_string_buffer(&mut self, ele: &[u8]) {
        unsafe {
            raw::RedisModule_DigestAddStringBuffer
                .expect("RedisModule_DigestAddStringBuffer is not available.")(
                self.dig,
                ele.as_ptr().cast::<c_char>(),
                ele.len(),
            )
        }
    }

    /// Similar to [`Digest::add_string_buffer`], but takes [`i64`].
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_DigestAddLongLong` is missing in redismodule.h
    pub fn add_long_long(&mut self, ll: c_longlong) {
        unsafe {
            raw::RedisModule_DigestAddLongLong
                .expect("RedisModule_DigestAddLongLong is not available.")(self.dig, ll)
        }
    }

    /// Ends the current sequence in this [`Digest`].
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_DigestEndSequence` is missing in redismodule.h
    pub fn end_sequence(&mut self) {
        unsafe {
            raw::RedisModule_DigestEndSequence
                .expect("RedisModule_DigestEndSequence is not available.")(self.dig)
        }
    }
}
