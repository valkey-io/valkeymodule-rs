use crate::{raw, ValkeyString};
use std::os::raw::c_char;
use std::ptr::null_mut;

impl ValkeyString {
    pub fn test<T: Into<Vec<u8>>>(data: T) -> ValkeyString {
        super::setup_test_shims();
        let data = Box::into_raw(Box::new(data.into()));
        let inner = data.cast::<raw::RedisModuleString>();
        ValkeyString::from_redis_module_string(null_mut(), inner)
    }
}

pub(super) extern "C" fn string_ptr_len(
    string: *const raw::RedisModuleString,
    len: *mut usize,
) -> *const c_char {
    let data = unsafe { &*string.cast::<Vec<u8>>() };
    unsafe {
        *len = data.len();
    }
    data.as_ptr().cast::<c_char>()
}

pub(super) extern "C" fn free_string(
    _ctx: *mut raw::RedisModuleCtx,
    string: *mut raw::RedisModuleString,
) {
    if !string.is_null() {
        drop(unsafe { Box::from_raw(string.cast::<Vec<u8>>()) });
    }
}
