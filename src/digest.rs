use crate::raw;
use std::os::raw::{c_int};

/// `Digest` is a high-level rust interface to the Valkey module C API 
/// abstracting away the raw C ffi calls.
pub struct Digest {
    pub dig: *mut raw::RedisModuleDigest,
}

impl Digest {
    pub const fn new(dig: *mut raw::RedisModuleDigest) -> Self {
        Self { dig }
    }

    pub fn key_name(&self) -> *mut raw::RedisModuleString {
        unsafe { (*self.dig).key }
    }

    pub fn db_id(&self) -> c_int {
        unsafe { (*self.dig).dbid }
    }
}

