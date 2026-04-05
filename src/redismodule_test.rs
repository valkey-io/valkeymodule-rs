use std::cmp::Ordering;
use std::os::raw::{c_char, c_int};
use std::sync::Once;

use crate::raw;
use crate::ValkeyString;

/// Heap-allocated backing store for a test `ValkeyString`.
/// The pointer is cast to `*mut RedisModuleString` so the existing
/// API surface (`try_as_str`, `as_slice`, `len`, etc.) works without
/// a running Valkey server.
#[repr(C)]
struct TestStringInner {
    data: Vec<u8>,
}

unsafe extern "C" fn test_string_ptr_len(
    str_: *const raw::RedisModuleString,
    len: *mut usize,
) -> *const c_char {
    let inner = str_ as *const TestStringInner;
    let data = &(*inner).data;
    if !len.is_null() {
        *len = data.len();
    }
    data.as_ptr() as *const c_char
}

unsafe extern "C" fn test_create_string(
    _ctx: *mut raw::RedisModuleCtx,
    ptr: *const c_char,
    len: usize,
) -> *mut raw::RedisModuleString {
    let data = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) }.to_vec();
    let inner = Box::new(TestStringInner { data });
    Box::into_raw(inner) as *mut raw::RedisModuleString
}

unsafe extern "C" fn test_free_string(
    _ctx: *mut raw::RedisModuleCtx,
    str_: *mut raw::RedisModuleString,
) {
    if !str_.is_null() {
        let _ = unsafe { Box::from_raw(str_ as *mut TestStringInner) };
    }
}

unsafe extern "C" fn test_retain_string(
    _ctx: *mut raw::RedisModuleCtx,
    _str: *mut raw::RedisModuleString,
) {
    // no-op in tests
}

unsafe extern "C" fn test_string_compare(
    a: *const raw::RedisModuleString,
    b: *const raw::RedisModuleString,
) -> c_int {
    let a = &(*(a as *const TestStringInner)).data;
    let b = &(*(b as *const TestStringInner)).data;
    match a.cmp(b) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

static INIT: Once = Once::new();

/// Install test shim function pointers so `ValkeyString` methods work
/// without the Valkey C runtime.
fn ensure_test_shims() {
    INIT.call_once(|| unsafe {
        raw::RedisModule_StringPtrLen = Some(test_string_ptr_len);
        raw::RedisModule_CreateString = Some(test_create_string);
        raw::RedisModule_FreeString = Some(test_free_string);
        raw::RedisModule_RetainString = Some(test_retain_string);
        raw::RedisModule_StringCompare = Some(test_string_compare);
    });
}

#[cfg(any(test, feature = "test-mocks"))]
impl ValkeyString {
    /// Create a `ValkeyString` that owns its bytes without a running Valkey
    /// server. Only available in test / `test-mocks` builds.
    ///
    /// The returned value supports `try_as_str`, `as_slice`, `len`, and
    /// `is_empty` — everything needed to pass it as a command argument in
    /// unit tests.
    #[cfg(any(test, feature = "test-mocks"))]
    pub fn create_for_test(s: &str) -> Self {
        ensure_test_shims();
        ValkeyString::create(None, s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessors_return_input_bytes() {
        let s = ValkeyString::create_for_test("hello");
        assert_eq!(s.try_as_str().unwrap(), "hello");
        assert_eq!(s.as_slice(), b"hello");
        assert_eq!(s.len(), 5);
        assert!(!s.is_empty());
    }

    #[test]
    fn empty_string_is_empty() {
        let s = ValkeyString::create_for_test("");
        assert_eq!(s.len(), 0);
        assert!(s.is_empty());
    }

    #[test]
    fn ordering_and_equality() {
        let a = ValkeyString::create_for_test("aaa");
        let b = ValkeyString::create_for_test("bbb");
        let a2 = ValkeyString::create_for_test("aaa");
        assert!(a < b);
        assert!(b > a);
        assert_eq!(a, a2);
    }
}
