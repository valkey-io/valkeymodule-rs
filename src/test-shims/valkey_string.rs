use crate::{raw, ValkeyString};
use std::cmp::Ordering;
use std::os::raw::c_char;
use std::ptr::null_mut;
use std::str::FromStr;
use std::sync::Arc;

impl ValkeyString {
    /// Creates a binary-safe `ValkeyString` for tests that run without a Valkey server.
    ///
    /// The bytes are stored in an `Arc<Vec<u8>>` and exposed through the same opaque pointer
    /// used by the Valkey module API. The installed shims manage that allocation until the last
    /// owning `ValkeyString` is dropped.
    pub fn test<T: Into<Vec<u8>>>(data: T) -> ValkeyString {
        // Install the process-wide callbacks before constructing a value that depends on them.
        super::setup_test_shims();
        let inner = into_raw_string(data.into());
        // The new allocation already represents one owner, so do not retain it again here.
        ValkeyString::from_redis_module_string(null_mut(), inner)
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
    // Match the defensive behavior expected by callers that may free a null value.
    if string.is_null() {
        return;
    }

    // SAFETY: The pointer came from `Arc::into_raw`, and each owner releases it exactly once.
    unsafe {
        Arc::decrement_strong_count(string.cast::<Vec<u8>>());
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
    // Retaining a null pointer has no useful effect and must not touch the Arc APIs.
    if string.is_null() {
        return;
    }

    // SAFETY: The pointer came from `Arc::into_raw` and still has a live strong reference.
    unsafe {
        Arc::increment_strong_count(string.cast::<Vec<u8>>());
    }
}

/// Parses a shim-backed string as a signed C `long long` value.
pub(super) extern "C" fn string_to_longlong(
    string: *const raw::RedisModuleString,
    value: *mut i64,
) -> libc::c_int {
    // Keep status and output-pointer handling consistent across numeric conversions.
    parse_string(string, value)
}

/// Parses a shim-backed string as an unsigned C `long long` value.
pub(super) extern "C" fn string_to_ulonglong(
    string: *const raw::RedisModuleString,
    value: *mut libc::c_ulonglong,
) -> libc::c_int {
    // Negative, overflowing, and otherwise invalid inputs are rejected by unsigned parsing.
    parse_string(string, value)
}

/// Parses a shim-backed string as a C `double` value.
pub(super) extern "C" fn string_to_double(
    string: *const raw::RedisModuleString,
    value: *mut f64,
) -> libc::c_int {
    // Use the same conversion path as the integer callbacks.
    parse_string(string, value)
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
    // Clone the bytes so the returned string can be owned and dropped independently.
    into_raw_string(data.clone())
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
    // Numeric representations accepted by Rust parsers must first be valid UTF-8.
    let Ok(data) = std::str::from_utf8(data) else {
        return raw::Status::Err as libc::c_int;
    };
    // Parsing failure maps directly to the module API's error status.
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
/// `string` must be non-null, aligned, and point to a live `Vec<u8>` previously passed through
/// `Arc::into_raw`. The returned reference must not outlive that allocation.
unsafe fn string_data<'a>(string: *const raw::RedisModuleString) -> &'a Vec<u8> {
    // SAFETY: The caller guarantees the pointer provenance, alignment, and lifetime above.
    unsafe { &*string.cast::<Vec<u8>>() }
}

/// Allocates an independently owned byte vector and erases its type for the module API.
fn into_raw_string(data: Vec<u8>) -> *mut raw::RedisModuleString {
    // Transfer one Arc strong reference into the raw pointer; `free_string` releases it later.
    Arc::into_raw(Arc::new(data))
        .cast_mut()
        .cast::<raw::RedisModuleString>()
}
