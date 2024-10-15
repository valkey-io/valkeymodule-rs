use crate::raw;
use std::os::raw::c_char;

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
    pub fn get_key_name(&self) -> *const raw::RedisModuleString {
        unsafe {
            match raw::RedisModule_GetKeyNameFromDigest {
                Some(get_key_name_from_digest) => get_key_name_from_digest(self.dig),
                None => panic!("RedisModule_GetKeyNameFromDigest is not available."),
            }
        }
    }

    /// Returns the database id of this [`Digest`].
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_GetDbIdFromDigest` is missing in redismodule.h
    pub fn get_db_id(&self) -> i32 {
        unsafe {
            match raw::RedisModule_GetDbIdFromDigest {
                Some(get_db_id_from_digest) => get_db_id_from_digest(self.dig),
                None => panic!("RedisModule_GetDbIdFromDigest is not available."),
            }
        }
    }

    /// Converts the long long input to a string and adds it to this [`Digest`].
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_DigestAddLongLong` is missing in redismodule.h
    pub fn add_long_long(&mut self, ll: i64) {
        unsafe {
            match raw::RedisModule_DigestAddLongLong {
                Some(digest_add_long_long) => digest_add_long_long(self.dig, ll),
                None => panic!("RedisModule_DigestAddLongLong is not available."),
            }
        }
    }

    /// Add a new element to this [`Digest`].
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_DigestAddStringBuffer` is missing in redismodule.h
    pub fn add_string_buffer(&mut self, ele: &[u8]) {
        unsafe {
            match raw::RedisModule_DigestAddStringBuffer {
                Some(digest_add_string_buffer) => {
                    digest_add_string_buffer(self.dig, ele.as_ptr().cast::<c_char>(), ele.len())
                }
                None => panic!("RedisModule_DigestAddStringBuffer is not available."),
            }
        }
    }

    /// Ends the current sequence in this [`Digest`].
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_DigestEndSequence` is missing in redismodule.h
    pub fn end_sequence(&mut self) {
        unsafe {
            match raw::RedisModule_DigestEndSequence {
                Some(digest_end_sequence) => digest_end_sequence(self.dig),
                None => panic!("RedisModule_DigestEndSequence is not available."),
            }
        }
    }
}
