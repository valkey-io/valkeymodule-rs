use crate::{raw, ValkeyString};
use std::cmp::Ordering;
use std::os::raw::c_char;
use std::ptr::null_mut;
use std::str::FromStr;
use std::sync::Arc;

pub(super) fn install() {
    // SAFETY: `setup_test_shims` calls this once after verifying the real API is uninitialized.
    unsafe {
        raw::RedisModule_StringPtrLen = Some(string_ptr_len);
        raw::RedisModule_FreeString = Some(free_string);
        raw::RedisModule_RetainString = Some(retain_string);
        raw::RedisModule_StringToLongLong = Some(string_to_longlong);
        raw::RedisModule_StringToULongLong = Some(string_to_ulonglong);
        raw::RedisModule_StringToDouble = Some(string_to_double);
        raw::RedisModule_CreateString = Some(create_string);
        raw::RedisModule_CreateStringFromString = Some(create_string_from_string);
        raw::RedisModule_StringCompare = Some(string_compare);
    }
}

type ShimString = Vec<u8>;

impl ValkeyString {
    /// Creates a binary-safe `ValkeyString` for tests that run without a Valkey server.
    ///
    /// The bytes are stored in an `Arc<Vec<u8>>` and exposed through the same opaque pointer
    /// used by the Valkey module API. The installed shims manage that allocation until the last
    /// owning `ValkeyString` is dropped.
    pub fn test<T: Into<Vec<u8>>>(data: T) -> ValkeyString {
        super::setup_test_shims();
        let inner = into_raw_string(data.into());
        // The new allocation already represents one owner, so do not retain it again here.
        Self::from_redis_module_string(null_mut(), inner)
    }
}

/// Returns the byte buffer and length of a shim-backed module string.
///
/// The returned pointer remains valid while at least one owner of `string` remains alive.
pub(super) extern "C" fn string_ptr_len(
    string: *const raw::RedisModuleString,
    len: *mut usize,
) -> *const c_char {
    // SAFETY: The shim is installed only for pointers created by `into_raw_string`.
    let data = unsafe { string_data(string) };
    // SAFETY: `ValkeyString` supplies a non-null pointer to writable length storage.
    unsafe {
        *len = data.len();
    }
    data.as_ptr().cast::<c_char>()
}

/// Releases one owner of a shim-backed module string.
///
/// The context is intentionally ignored because test strings are not associated with a real
/// Valkey context. A null string is accepted as a no-op.
pub(super) extern "C" fn free_string(
    _ctx: *mut raw::RedisModuleCtx,
    string: *mut raw::RedisModuleString,
) {
    if string.is_null() {
        return;
    }

    // SAFETY: The pointer came from `Arc::into_raw`, and each owner releases it exactly once.
    unsafe {
        Arc::decrement_strong_count(string.cast::<ShimString>());
    }
}

/// Adds one owner to a shim-backed module string.
///
/// The context is intentionally ignored because ownership is represented by the `Arc` strong
/// count rather than Valkey's context-bound memory management.
pub(super) extern "C" fn retain_string(
    _ctx: *mut raw::RedisModuleCtx,
    string: *mut raw::RedisModuleString,
) {
    if string.is_null() {
        return;
    }

    // SAFETY: The pointer came from `Arc::into_raw` and still has a live strong reference.
    unsafe {
        Arc::increment_strong_count(string.cast::<ShimString>());
    }
}

/// Parses a shim-backed string as a signed C `long long` value.
pub(super) extern "C" fn string_to_longlong(
    string: *const raw::RedisModuleString,
    value: *mut i64,
) -> libc::c_int {
    parse_string(string, value)
}

/// Parses a shim-backed string as an unsigned C `long long` value.
pub(super) extern "C" fn string_to_ulonglong(
    string: *const raw::RedisModuleString,
    value: *mut libc::c_ulonglong,
) -> libc::c_int {
    parse_string(string, value)
}

/// Parses a shim-backed string as a C `double` value.
pub(super) extern "C" fn string_to_double(
    string: *const raw::RedisModuleString,
    value: *mut f64,
) -> libc::c_int {
    parse_string(string, value)
}

/// Creates a shim-backed module string by copying `len` bytes from `ptr`.
pub(super) extern "C" fn create_string(
    _ctx: *mut raw::RedisModuleCtx,
    ptr: *const c_char,
    len: usize,
) -> *mut raw::RedisModuleString {
    if ptr.is_null() {
        return if len == 0 {
            into_raw_string(Vec::new())
        } else {
            null_mut()
        };
    }

    // SAFETY: The callback contract requires `ptr` to reference at least `len` readable bytes.
    let data = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), len) };
    into_raw_string(data.to_vec())
}

/// Creates an independently owned copy of a shim-backed module string.
///
/// This models `RedisModule_CreateStringFromString`, which creates a new string rather than
/// retaining the original allocation.
pub(super) extern "C" fn create_string_from_string(
    _ctx: *mut raw::RedisModuleCtx,
    string: *const raw::RedisModuleString,
) -> *mut raw::RedisModuleString {
    // SAFETY: `string` is expected to be an opaque pointer created by this shim.
    let data = unsafe { string_data(string) };
    into_raw_string(data.to_vec())
}

/// Compares two shim-backed strings byte by byte.
///
/// Returns `-1`, `0`, or `1` to match the Valkey module API contract.
pub(super) extern "C" fn string_compare(
    left: *const raw::RedisModuleString,
    right: *const raw::RedisModuleString,
) -> libc::c_int {
    // SAFETY: Both pointers are expected to refer to live values allocated by this shim.
    let left = unsafe { string_data(left) };
    let right = unsafe { string_data(right) };

    // `Vec<u8>` ordering is lexicographic, matching Valkey's binary string comparison.
    match left.cmp(right) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Parses the bytes in a shim-backed string and writes the result through an FFI out pointer.
///
/// The output is left unchanged when UTF-8 validation or parsing fails.
fn parse_string<T: FromStr>(string: *const raw::RedisModuleString, value: *mut T) -> libc::c_int {
    // SAFETY: Numeric callbacks receive pointers produced by `into_raw_string`.
    let data = unsafe { string_data(string) };
    let Ok(data) = std::str::from_utf8(data) else {
        return raw::Status::Err as libc::c_int;
    };
    let Ok(data) = data.parse::<T>() else {
        return raw::Status::Err as libc::c_int;
    };

    // SAFETY: The callback contract requires `value` to point to writable storage for `T`.
    unsafe {
        *value = data;
    }
    raw::Status::Ok as libc::c_int
}

/// Borrows the byte vector hidden behind an opaque module-string pointer.
///
/// # Safety
///
/// `string` must be non-null, aligned, and point to a live `ShimString` previously passed through
/// `Arc::into_raw`. The returned slice must not outlive that allocation.
pub(super) unsafe fn string_data<'a>(string: *const raw::RedisModuleString) -> &'a [u8] {
    // SAFETY: The caller guarantees the pointer provenance, alignment, and lifetime above.
    unsafe { (&*string.cast::<ShimString>()).as_slice() }
}

/// Allocates an independently owned byte vector and erases its type for the module API.
fn into_raw_string(data: ShimString) -> *mut raw::RedisModuleString {
    // Transfer one Arc strong reference into the raw pointer; `free_string` releases it later.
    Arc::into_raw(Arc::new(data))
        .cast_mut()
        .cast::<raw::RedisModuleString>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_reads_and_drops_string() {
        let string = ValkeyString::test("value");

        assert_eq!(string.as_slice(), b"value");
        assert_eq!(string.len(), 5);

        drop(string);
    }

    #[test]
    fn preserves_arbitrary_binary_data() {
        let bytes = vec![0x00, 0xff, b'a'];
        let string = ValkeyString::test(bytes.clone());

        assert_eq!(string.as_slice(), bytes);
    }

    #[test]
    fn context_creates_string() {
        let context = crate::Context::test();
        let data = "é日";
        let string = context.create_string(data);

        assert_eq!(string.as_slice(), data.as_bytes());
        assert_eq!(string.len(), data.len());
    }

    #[test]
    fn create_string_copies_binary_input() {
        let data = [0x00, 0xff, b'a'];
        let inner = create_string(null_mut(), data.as_ptr().cast::<c_char>(), data.len());
        let string = ValkeyString::from_redis_module_string(null_mut(), inner);

        assert_eq!(string.as_slice(), data);
    }

    #[test]
    fn create_string_accepts_null_for_empty_input() {
        let inner = create_string(null_mut(), std::ptr::null(), 0);
        let string = ValkeyString::from_redis_module_string(null_mut(), inner);

        assert!(string.is_empty());
    }

    #[test]
    fn create_string_rejects_null_for_nonempty_input() {
        assert!(create_string(null_mut(), std::ptr::null(), 1).is_null());
    }

    #[test]
    fn reports_multibyte_utf8_length_in_bytes() {
        let data = "é日";
        let string = ValkeyString::test(data);

        assert_eq!(string.len(), data.len());
        assert_eq!(string.as_slice(), data.as_bytes());
    }

    #[test]
    fn retained_string_survives_original_drop() {
        let original = ValkeyString::test("value");
        let retained = ValkeyString::new(None, original.inner);

        drop(original);

        assert_eq!(retained.as_slice(), b"value");
    }

    #[test]
    fn original_string_survives_retained_drop() {
        let original = ValkeyString::test("value");
        let retained = ValkeyString::new(None, original.inner);

        drop(retained);

        assert_eq!(original.as_slice(), b"value");
    }

    #[test]
    fn clone_has_independent_allocation() {
        let original = ValkeyString::test("value");
        let cloned = original.clone();

        assert_ne!(original.inner, cloned.inner);
        drop(original);

        assert_eq!(cloned.as_slice(), b"value");
    }

    #[test]
    fn compares_strings_by_binary_value() {
        let lower = ValkeyString::test([0x00, 0xff]);
        let equal = ValkeyString::test([0x00, 0xff]);
        let greater = ValkeyString::test([0x01]);

        assert_eq!(lower, equal);
        assert!(lower < greater);
        assert!(greater > lower);
    }

    #[test]
    fn parses_signed_integer() {
        let string = ValkeyString::test("-42");

        assert_eq!(string.parse_integer().expect("integer should parse"), -42);
    }

    #[test]
    fn parses_full_unsigned_integer_range() {
        let string = ValkeyString::test(u64::MAX.to_string());

        assert_eq!(
            string
                .parse_unsigned_integer()
                .expect("unsigned integer should parse"),
            u64::MAX
        );
    }

    #[test]
    fn parses_double() {
        let string = ValkeyString::test("42.5");

        assert_eq!(string.parse_float().expect("double should parse"), 42.5);
    }

    #[test]
    fn parse_failure_leaves_output_unchanged() {
        let string = ValkeyString::test([0xff]);
        let mut value = 7;

        let status =
            unsafe { raw::RedisModule_StringToLongLong.unwrap()(string.inner, &mut value) };

        assert_eq!(status, raw::Status::Err as libc::c_int);
        assert_eq!(value, 7);
    }

    #[test]
    fn invalid_numeric_syntax_leaves_output_unchanged() {
        let string = ValkeyString::test("not-a-number");
        let mut value = 7;

        let status =
            unsafe { raw::RedisModule_StringToLongLong.unwrap()(string.inner, &mut value) };

        assert_eq!(status, raw::Status::Err as libc::c_int);
        assert_eq!(value, 7);
    }

    #[test]
    fn free_string_accepts_null() {
        free_string(null_mut(), null_mut());
    }

    #[test]
    fn retain_string_accepts_null() {
        retain_string(null_mut(), null_mut());
    }
}
