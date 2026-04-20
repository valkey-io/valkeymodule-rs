use std::cmp::Ordering;
use std::os::raw::{c_char, c_double, c_int, c_longlong};
use std::slice;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Once;

use crate::raw;
use crate::ValkeyString;

/// Heap-allocated backing store for a test `ValkeyString`.
/// The pointer is cast to `*mut RedisModuleString` so the existing
/// API surface (`try_as_str`, `as_slice`, `len`, etc.) works without
/// a running Valkey server.
#[repr(C)]
struct TestStringInner {
    refcount: AtomicUsize,
    data: Vec<u8>,
}

impl TestStringInner {
    fn new(data: Vec<u8>) -> Self {
        // Start at one reference to match a freshly created module string.
        Self {
            refcount: AtomicUsize::new(1),
            data,
        }
    }
}

fn test_string_inner<'a>(str_: *const raw::RedisModuleString) -> &'a TestStringInner {
    // Test shims only pass pointers created from `TestStringInner` allocations.
    unsafe { &*str_.cast::<TestStringInner>() }
}

fn test_string_inner_mut<'a>(str_: *mut raw::RedisModuleString) -> &'a mut TestStringInner {
    // Mutable access is only used for those same test-owned string allocations.
    unsafe { &mut *str_.cast::<TestStringInner>() }
}

fn allocate_test_string(data: Vec<u8>) -> *mut raw::RedisModuleString {
    // Wrap test-owned bytes in the same opaque pointer shape the raw API uses.
    let test_string = TestStringInner::new(data);
    let boxed_test_string = Box::new(test_string);
    let test_string_ptr = Box::into_raw(boxed_test_string);

    test_string_ptr.cast()
}

fn parse_test_string<T: FromStr>(str_: *const raw::RedisModuleString) -> Option<T> {
    // Shared parser for shim APIs that first require valid UTF-8 input bytes.
    let test_string = test_string_inner(str_);
    let string_bytes = &test_string.data;
    let string_utf8 = str::from_utf8(string_bytes);

    string_utf8.ok().and_then(|s| s.parse::<T>().ok())
}

fn write_optional_output<T>(out: *mut T, value: T) {
    // Match Valkey's out-parameter convention: write only when the caller supplies one.
    if !out.is_null() {
        unsafe {
            *out = value;
        }
    }
}

extern "C" fn test_string_ptr_len(
    str_: *const raw::RedisModuleString,
    len: *mut usize,
) -> *const c_char {
    // The real Valkey API returns a raw pointer to the string bytes and writes
    // the byte length into the provided out-parameter when one is supplied.
    let test_string = test_string_inner(str_);
    let string_bytes = &test_string.data;
    let string_len = string_bytes.len();

    if !len.is_null() {
        unsafe {
            *len = string_len;
        }
    }

    let string_bytes_ptr = string_bytes.as_ptr();
    string_bytes_ptr.cast::<c_char>()
}

extern "C" fn test_create_string(
    _ctx: *mut raw::RedisModuleCtx,
    ptr: *const c_char,
    len: usize,
) -> *mut raw::RedisModuleString {
    // The real Valkey API accepts a raw C pointer/length pair, copies those
    // bytes into a new module-owned string allocation, and returns an opaque
    // `RedisModuleString` pointer to that owned storage.
    let input_bytes_ptr = ptr.cast::<u8>();
    let input_bytes = unsafe { slice::from_raw_parts(input_bytes_ptr, len) };

    allocate_test_string(input_bytes.to_vec())
}

extern "C" fn test_create_string_from_string(
    _ctx: *mut raw::RedisModuleCtx,
    str_: *const raw::RedisModuleString,
) -> *mut raw::RedisModuleString {
    // The real Valkey API returns a new module string allocation containing a
    // copy of the source string bytes, so the cloned string can outlive and
    // diverge from the original.
    allocate_test_string(test_string_inner(str_).data.clone())
}

extern "C" fn test_free_string(_ctx: *mut raw::RedisModuleCtx, str_: *mut raw::RedisModuleString) {
    if !str_.is_null() {
        // `ValkeyString::safe_clone` can retain the original allocation, so the
        // test shim needs refcounted destruction instead of freeing eagerly on
        // every drop.
        let test_string = test_string_inner_mut(str_);
        let previous_refcount = test_string.refcount.fetch_sub(1, AtomicOrdering::AcqRel);
        let should_drop_allocation = previous_refcount == 1;

        if should_drop_allocation {
            let test_string_ptr = str_.cast::<TestStringInner>();
            let _owned_test_string = unsafe { Box::from_raw(test_string_ptr) };
        }
    }
}

extern "C" fn test_retain_string(
    _ctx: *mut raw::RedisModuleCtx,
    str_: *mut raw::RedisModuleString,
) {
    if !str_.is_null() {
        // The real Valkey API retains the existing module string allocation by
        // incrementing its reference count, so this test shim mirrors that
        // behavior on the Rust-owned backing allocation.
        let test_string = test_string_inner_mut(str_);
        let refcount = &test_string.refcount;

        refcount.fetch_add(1, AtomicOrdering::AcqRel);
    }
}

extern "C" fn test_string_compare(
    a: *const raw::RedisModuleString,
    b: *const raw::RedisModuleString,
) -> c_int {
    // The real Valkey API compares the underlying string bytes and returns a
    // negative value, zero, or a positive value depending on the ordering.
    let left_test_string = test_string_inner(a);
    let right_test_string = test_string_inner(b);
    let left_bytes = &left_test_string.data;
    let right_bytes = &right_test_string.data;
    let ordering = left_bytes.cmp(right_bytes);

    match ordering {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

extern "C" fn test_string_append_buffer(
    _ctx: *mut raw::RedisModuleCtx,
    str_: *mut raw::RedisModuleString,
    buf: *const c_char,
    len: usize,
) -> c_int {
    // The real Valkey API appends raw bytes into the existing string buffer.
    // Reconstruct that byte slice from the incoming C pointer/length pair, then
    // mutate the test-owned backing store in place.
    let appended_bytes_ptr = buf.cast::<u8>();
    let appended_bytes = unsafe { slice::from_raw_parts(appended_bytes_ptr, len) };
    let test_string = test_string_inner_mut(str_);
    let existing_bytes = &mut test_string.data;

    existing_bytes.extend_from_slice(appended_bytes);
    raw::REDISMODULE_OK as c_int
}

extern "C" fn test_string_to_longlong(
    str_: *const raw::RedisModuleString,
    ll: *mut c_longlong,
) -> c_int {
    // The real Valkey API parses the string bytes as a signed integer and, on
    // success, writes the parsed value into the provided out-parameter.
    match parse_test_string::<i64>(str_) {
        Some(value) => {
            write_optional_output(ll, value as c_longlong);
            raw::REDISMODULE_OK as c_int
        }
        None => raw::REDISMODULE_ERR as c_int,
    }
}

extern "C" fn test_string_to_double(
    str_: *const raw::RedisModuleString,
    d: *mut c_double,
) -> c_int {
    // The real Valkey API parses the string bytes as a floating-point number
    // and, on success, writes the parsed value into the provided
    // out-parameter.
    match parse_test_string::<f64>(str_) {
        Some(value) => {
            write_optional_output(d, value as c_double);
            raw::REDISMODULE_OK as c_int
        }
        None => raw::REDISMODULE_ERR as c_int,
    }
}

static INIT: Once = Once::new();

/// Install test shim function pointers so `ValkeyString` methods work
/// without the Valkey C runtime.
fn ensure_test_shims() {
    INIT.call_once(|| unsafe {
        let create_string = raw::RedisModule_CreateString;
        if create_string.is_none() {
            // Only install the shim when the raw API has not already been
            // initialized by a real Valkey context.
            raw::RedisModule_StringPtrLen = Some(test_string_ptr_len);
            raw::RedisModule_CreateString = Some(test_create_string);
            raw::RedisModule_CreateStringFromString = Some(test_create_string_from_string);
            raw::RedisModule_FreeString = Some(test_free_string);
            raw::RedisModule_RetainString = Some(test_retain_string);
            raw::RedisModule_StringCompare = Some(test_string_compare);
            raw::RedisModule_StringAppendBuffer = Some(test_string_append_buffer);
            raw::RedisModule_StringToLongLong = Some(test_string_to_longlong);
            raw::RedisModule_StringToDouble = Some(test_string_to_double);
        }
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
    pub fn create_for_test<T: Into<Vec<u8>>>(s: T) -> Self {
        // Install the shim lazily so tests can opt in only when they need it.
        ensure_test_shims();
        ValkeyString::create(None, s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Borrow;

    #[test]
    fn create_for_test_installs_all_required_string_shims() {
        ensure_test_shims();

        let create = unsafe { raw::RedisModule_CreateString };
        let create_from_string = unsafe { raw::RedisModule_CreateStringFromString };
        let free = unsafe { raw::RedisModule_FreeString };
        let retain = unsafe { raw::RedisModule_RetainString };
        let ptr_len = unsafe { raw::RedisModule_StringPtrLen };
        let compare = unsafe { raw::RedisModule_StringCompare };
        let append = unsafe { raw::RedisModule_StringAppendBuffer };
        let to_longlong = unsafe { raw::RedisModule_StringToLongLong };
        let to_double = unsafe { raw::RedisModule_StringToDouble };

        assert!(create.is_some());
        assert!(create_from_string.is_some());
        assert!(free.is_some());
        assert!(retain.is_some());
        assert!(ptr_len.is_some());
        assert!(compare.is_some());
        assert!(append.is_some());
        assert!(to_longlong.is_some());
        assert!(to_double.is_some());
    }

    #[test]
    fn test_string_inner_returns_original_allocation() {
        let s = ValkeyString::create_for_test("hello");
        let inner = test_string_inner(s.inner);

        assert_eq!(
            inner as *const TestStringInner,
            s.inner.cast::<TestStringInner>()
        );
        assert_eq!(inner.data, b"hello");
    }

    #[test]
    fn test_string_inner_mut_updates_backing_bytes() {
        let s = ValkeyString::create_for_test("hello");

        let inner = test_string_inner_mut(s.inner);
        inner.data.extend_from_slice(b"!");

        assert_eq!(s.as_slice(), b"hello!");
        assert_eq!(s.try_as_str().unwrap(), "hello!");
    }

    #[test]
    fn accessors_return_input_bytes() {
        let s = ValkeyString::create_for_test("hello");
        assert_eq!(s.try_as_str().unwrap(), "hello");
        assert_eq!(s.as_slice(), b"hello");
        assert_eq!(s.len(), 5);
        assert!(!s.is_empty());
    }

    #[test]
    fn create_for_test_accepts_byte_buffers() {
        let s = ValkeyString::create_for_test(vec![0x66, 0x6f, 0x6f]);

        assert_eq!(s.as_slice(), b"foo");
        assert_eq!(s.try_as_str().unwrap(), "foo");
    }

    #[test]
    fn invalid_utf8_is_preserved_in_slice_accessors() {
        let s = ValkeyString::create_for_test(vec![0xff, b'o', b'o']);

        assert_eq!(s.as_slice(), &[0xff, b'o', b'o']);
        assert!(matches!(
            s.try_as_str(),
            Err(crate::ValkeyError::Str("Couldn't parse as UTF-8 string"))
        ));
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

    #[test]
    fn clone_creates_independent_string() {
        let mut s = ValkeyString::create_for_test("41");
        let cloned = s.clone();

        s.append(".5");

        assert_eq!(cloned.try_as_str().unwrap(), "41");
        assert_eq!(s.try_as_str().unwrap(), "41.5");
    }

    #[test]
    fn append_returns_ok_and_updates_bytes() {
        let mut s = ValkeyString::create_for_test("foo");

        assert_eq!(s.append("bar"), raw::Status::Ok);
        assert_eq!(s.as_slice(), b"foobar");
    }

    #[test]
    fn integer_and_float_parsing_work() {
        let integer = ValkeyString::create_for_test("41");
        let float = ValkeyString::create_for_test("41.5");

        assert_eq!(integer.parse_integer().unwrap(), 41);
        assert_eq!(integer.parse_unsigned_integer().unwrap(), 41);
        assert_eq!(float.parse_float().unwrap(), 41.5);
    }

    #[test]
    fn parsing_errors_match_expected_messages() {
        let invalid_int = ValkeyString::create_for_test("abc");
        let negative = ValkeyString::create_for_test("-1");
        let invalid_float = ValkeyString::create_for_test("abc");

        assert!(matches!(
            invalid_int.parse_integer(),
            Err(crate::ValkeyError::Str("Couldn't parse as integer"))
        ));
        assert!(matches!(
            negative.parse_unsigned_integer(),
            Err(crate::ValkeyError::Str(
                "Couldn't parse negative number as unsigned integer"
            ))
        ));
        assert!(matches!(
            invalid_float.parse_float(),
            Err(crate::ValkeyError::Str("Couldn't parse as float"))
        ));
    }

    #[test]
    fn safe_clone_keeps_original_alive() {
        let s = ValkeyString::create_for_test("hello");
        let cloned = s.safe_clone(&crate::Context::dummy());

        drop(s);

        assert_eq!(cloned.try_as_str().unwrap(), "hello");
    }

    #[test]
    fn borrow_returns_placeholder_for_invalid_utf8() {
        let s = ValkeyString::create_for_test(vec![0xff, b'o']);

        assert_eq!(
            <ValkeyString as Borrow<str>>::borrow(&s),
            "<Invalid UTF-8 data>"
        );
    }
}
