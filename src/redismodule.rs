use std::borrow::Borrow;
use std::ffi::CString;
use std::fmt::Display;
use std::ops::Deref;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr::{null_mut, NonNull};
use std::slice;
use std::str;
use std::str::Utf8Error;
use std::string::FromUtf8Error;
use std::{fmt, ptr};

use serde::de::{Error, SeqAccess};

pub use crate::raw;
pub use crate::rediserror::ValkeyError;
pub use crate::redisvalue::ValkeyValue;
use crate::Context;

/// A short-hand type that stores a [std::result::Result] with custom
/// type and [RedisError].
pub type ValkeyResult<T = ValkeyValue> = Result<T, ValkeyError>;
/// A [RedisResult] with [ValkeyValue].
pub type ValkeyValueResult = ValkeyResult<ValkeyValue>;

impl From<ValkeyValue> for ValkeyValueResult {
    fn from(v: ValkeyValue) -> Self {
        Ok(v)
    }
}

impl From<ValkeyError> for ValkeyValueResult {
    fn from(v: ValkeyError) -> Self {
        Err(v)
    }
}

pub const VALKEY_OK: ValkeyValueResult = Ok(ValkeyValue::SimpleStringStatic("OK"));
pub const TYPE_METHOD_VERSION: u64 = raw::REDISMODULE_TYPE_METHOD_VERSION as u64;
pub const AUTH_HANDLED: i32 = raw::REDISMODULE_AUTH_HANDLED as i32;
pub const AUTH_NOT_HANDLED: i32 = raw::REDISMODULE_AUTH_NOT_HANDLED as i32;

pub trait NextArg {
    fn next_arg(&mut self) -> Result<ValkeyString, ValkeyError>;
    fn next_string(&mut self) -> Result<String, ValkeyError>;
    fn next_str<'a>(&mut self) -> Result<&'a str, ValkeyError>;
    fn next_i64(&mut self) -> Result<i64, ValkeyError>;
    fn next_u64(&mut self) -> Result<u64, ValkeyError>;
    fn next_f64(&mut self) -> Result<f64, ValkeyError>;
    fn done(&mut self) -> Result<(), ValkeyError>;
}

impl<T> NextArg for T
where
    T: Iterator<Item = ValkeyString>,
{
    #[inline]
    fn next_arg(&mut self) -> Result<ValkeyString, ValkeyError> {
        self.next().ok_or(ValkeyError::WrongArity)
    }

    #[inline]
    fn next_string(&mut self) -> Result<String, ValkeyError> {
        self.next()
            .map_or(Err(ValkeyError::WrongArity), |v| Ok(v.to_string_lossy()))
    }

    #[inline]
    fn next_str<'a>(&mut self) -> Result<&'a str, ValkeyError> {
        self.next()
            .map_or(Err(ValkeyError::WrongArity), |v| v.try_as_str())
    }

    #[inline]
    fn next_i64(&mut self) -> Result<i64, ValkeyError> {
        self.next()
            .map_or(Err(ValkeyError::WrongArity), |v| v.parse_integer())
    }

    #[inline]
    fn next_u64(&mut self) -> Result<u64, ValkeyError> {
        self.next()
            .map_or(Err(ValkeyError::WrongArity), |v| v.parse_unsigned_integer())
    }

    #[inline]
    fn next_f64(&mut self) -> Result<f64, ValkeyError> {
        self.next()
            .map_or(Err(ValkeyError::WrongArity), |v| v.parse_float())
    }

    /// Return an error if there are any more arguments
    #[inline]
    fn done(&mut self) -> Result<(), ValkeyError> {
        self.next().map_or(Ok(()), |_| Err(ValkeyError::WrongArity))
    }
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn decode_args(
    ctx: *mut raw::RedisModuleCtx,
    argv: *mut *mut raw::RedisModuleString,
    argc: c_int,
) -> Vec<ValkeyString> {
    if argv.is_null() {
        return Vec::new();
    }
    unsafe { slice::from_raw_parts(argv, argc as usize) }
        .iter()
        .map(|&arg| ValkeyString::new(NonNull::new(ctx), arg))
        .collect()
}

///////////////////////////////////////////////////

#[derive(Debug)]
pub struct ValkeyString {
    ctx: *mut raw::RedisModuleCtx,
    pub inner: *mut raw::RedisModuleString,
}

impl ValkeyString {
    pub(crate) fn take(mut self) -> *mut raw::RedisModuleString {
        let inner = self.inner;
        self.inner = std::ptr::null_mut();
        inner
    }

    pub fn new(
        ctx: Option<NonNull<raw::RedisModuleCtx>>,
        inner: *mut raw::RedisModuleString,
    ) -> Self {
        let ctx = ctx.map_or(std::ptr::null_mut(), |v| v.as_ptr());
        raw::string_retain_string(ctx, inner);
        Self { ctx, inner }
    }

    /// In general, [RedisModuleString] is none atomic ref counted object.
    /// So it is not safe to clone it if Valkey GIL is not held.
    /// [Self::safe_clone] gets a context reference which indicates that Valkey GIL is held.
    pub fn safe_clone(&self, _ctx: &Context) -> Self {
        // RedisString are *not* atomic ref counted, so we must get a lock indicator to clone them.
        // Alos notice that Valkey allows us to create RedisModuleString with NULL context
        // so we use [std::ptr::null_mut()] instead of the curren RedisString context.
        // We do this because we can not promise the new RedisString will not outlive the current
        // context and we want them to be independent.
        raw::string_retain_string(ptr::null_mut(), self.inner);
        Self {
            ctx: ptr::null_mut(),
            inner: self.inner,
        }
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn create<T: Into<Vec<u8>>>(ctx: Option<NonNull<raw::RedisModuleCtx>>, s: T) -> Self {
        let ctx = ctx.map_or(std::ptr::null_mut(), |v| v.as_ptr());
        let str = CString::new(s).unwrap();
        let inner = unsafe {
            raw::RedisModule_CreateString.unwrap()(ctx, str.as_ptr(), str.as_bytes().len())
        };

        Self { ctx, inner }
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn create_from_slice(ctx: *mut raw::RedisModuleCtx, s: &[u8]) -> Self {
        let inner = unsafe {
            raw::RedisModule_CreateString.unwrap()(ctx, s.as_ptr().cast::<c_char>(), s.len())
        };

        Self { ctx, inner }
    }

    /// Creates a ValkeyString from a &str and retains it.  This is useful in cases where Modules need to pass ownership of a ValkeyString to the core engine without it being freed when we drop a ValkeyString
    pub fn create_and_retain(arg: &str) -> ValkeyString {
        let arg = ValkeyString::create(None, arg);
        raw::string_retain_string(null_mut(), arg.inner);
        arg
    }

    pub const fn from_redis_module_string(
        ctx: *mut raw::RedisModuleCtx,
        inner: *mut raw::RedisModuleString,
    ) -> Self {
        // Need to avoid string_retain_string
        Self { ctx, inner }
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn from_ptr<'a>(ptr: *const raw::RedisModuleString) -> Result<&'a str, Utf8Error> {
        str::from_utf8(Self::string_as_slice(ptr))
    }

    pub fn append(&mut self, s: &str) -> raw::Status {
        raw::string_append_buffer(self.ctx, self.inner, s)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        let mut len: usize = 0;
        raw::string_ptr_len(self.inner, &mut len);
        len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        let mut len: usize = 0;
        raw::string_ptr_len(self.inner, &mut len);
        len == 0
    }

    pub fn try_as_str<'a>(&self) -> Result<&'a str, ValkeyError> {
        Self::from_ptr(self.inner).map_err(|_| ValkeyError::Str("Couldn't parse as UTF-8 string"))
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        Self::string_as_slice(self.inner)
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn string_as_slice<'a>(ptr: *const raw::RedisModuleString) -> &'a [u8] {
        let mut len: libc::size_t = 0;
        let bytes = unsafe { raw::RedisModule_StringPtrLen.unwrap()(ptr, &mut len) };

        unsafe { slice::from_raw_parts(bytes.cast::<u8>(), len) }
    }

    /// Performs lossy conversion of a `RedisString` into an owned `String. This conversion
    /// will replace any invalid UTF-8 sequences with U+FFFD REPLACEMENT CHARACTER, which
    /// looks like this: �.
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_StringPtrLen` is missing in redismodule.h
    #[must_use]
    pub fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(self.as_slice()).into_owned()
    }

    pub fn parse_unsigned_integer(&self) -> Result<u64, ValkeyError> {
        let mut val: u64 = 0;
        match raw::string_to_ulonglong(self.inner, &mut val) {
            raw::Status::Ok => Ok(val),
            raw::Status::Err => Err(ValkeyError::Str("Couldn't parse as unsigned integer")),
        }
    }

    pub fn parse_integer(&self) -> Result<i64, ValkeyError> {
        let mut val: i64 = 0;
        match raw::string_to_longlong(self.inner, &mut val) {
            raw::Status::Ok => Ok(val),
            raw::Status::Err => Err(ValkeyError::Str("Couldn't parse as integer")),
        }
    }

    pub fn parse_float(&self) -> Result<f64, ValkeyError> {
        let mut val: f64 = 0.0;
        match raw::string_to_double(self.inner, &mut val) {
            raw::Status::Ok => Ok(val),
            raw::Status::Err => Err(ValkeyError::Str("Couldn't parse as float")),
        }
    }

    // TODO: Valkey allows storing and retrieving any arbitrary bytes.
    // However rust's String and str can only store valid UTF-8.
    // Implement these to allow non-utf8 bytes to be consumed:
    // pub fn into_bytes(self) -> Vec<u8> {}
    // pub fn as_bytes(&self) -> &[u8] {}
}

impl Drop for ValkeyString {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            unsafe {
                raw::RedisModule_FreeString.unwrap()(self.ctx, self.inner);
            }
        }
    }
}

impl PartialEq for ValkeyString {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for ValkeyString {}

impl PartialOrd for ValkeyString {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ValkeyString {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        raw::string_compare(self.inner, other.inner)
    }
}

impl core::hash::Hash for ValkeyString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

impl Display for ValkeyString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string_lossy())
    }
}

impl Borrow<str> for ValkeyString {
    fn borrow(&self) -> &str {
        // RedisString might not be UTF-8 safe
        self.try_as_str().unwrap_or("<Invalid UTF-8 data>")
    }
}

impl Clone for ValkeyString {
    fn clone(&self) -> Self {
        let inner =
            // Valkey allows us to create RedisModuleString with NULL context
            // so we use [std::ptr::null_mut()] instead of the curren RedisString context.
            // We do this because we can not promise the new RedisString will not outlive the current
            // context and we want them to be independent.
            unsafe { raw::RedisModule_CreateStringFromString.unwrap()(ptr::null_mut(), self.inner) };
        Self::from_redis_module_string(ptr::null_mut(), inner)
    }
}

impl From<ValkeyString> for String {
    fn from(rs: ValkeyString) -> Self {
        rs.to_string_lossy()
    }
}

impl Deref for ValkeyString {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl From<ValkeyString> for Vec<u8> {
    fn from(rs: ValkeyString) -> Self {
        rs.as_slice().to_vec()
    }
}

impl serde::Serialize for ValkeyString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(self.as_slice())
    }
}

struct RedisStringVisitor;

impl<'de> serde::de::Visitor<'de> for RedisStringVisitor {
    type Value = ValkeyString;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("A bytes buffer")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(ValkeyString::create(None, v))
    }

    fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let mut v = if let Some(size_hint) = visitor.size_hint() {
            Vec::with_capacity(size_hint)
        } else {
            Vec::new()
        };
        while let Some(elem) = visitor.next_element()? {
            v.push(elem);
        }

        Ok(ValkeyString::create(None, v.as_slice()))
    }
}

impl<'de> serde::Deserialize<'de> for ValkeyString {
    fn deserialize<D>(deserializer: D) -> Result<ValkeyString, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_bytes(RedisStringVisitor)
    }
}

///////////////////////////////////////////////////

#[derive(Debug)]
pub struct RedisBuffer {
    buffer: *mut c_char,
    len: usize,
}

impl RedisBuffer {
    pub const fn new(buffer: *mut c_char, len: usize) -> Self {
        Self { buffer, len }
    }

    pub fn to_string(&self) -> Result<String, FromUtf8Error> {
        String::from_utf8(self.as_ref().to_vec())
    }
}

impl AsRef<[u8]> for RedisBuffer {
    fn as_ref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.buffer as *const u8, self.len) }
    }
}

impl Drop for RedisBuffer {
    fn drop(&mut self) {
        unsafe {
            raw::RedisModule_Free.unwrap()(self.buffer.cast::<c_void>());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_clone_keeps_string_alive_after_original_is_dropped() {
        let context = Context::test();
        let original = ValkeyString::test("value");
        let cloned = original.safe_clone(&context);

        assert_eq!(cloned.inner, original.inner);
        drop(original);

        assert_eq!(cloned.as_slice(), b"value");
    }

    #[test]
    fn create_and_retain_keeps_the_transferred_reference_alive() {
        let _context = Context::test();
        let string = ValkeyString::create_and_retain("value");
        let inner = string.inner;

        assert_eq!(string.as_slice(), b"value");
        drop(string);

        // SAFETY: `create_and_retain` creates a second shim-backed reference for the caller to
        // transfer to Valkey. This releases that reference after the test's owner was dropped.
        unsafe {
            raw::RedisModule_FreeString.unwrap()(ptr::null_mut(), inner);
        }
    }

    #[test]
    fn create_from_slice_preserves_binary_data() {
        let _context = Context::test();
        let string = ValkeyString::create_from_slice(ptr::null_mut(), &[0, 0xff, b'a']);

        assert_eq!(string.as_slice(), &[0, 0xff, b'a']);
    }

    #[test]
    fn empty_string_is_empty() {
        let string = ValkeyString::test("");

        assert!(string.is_empty());
    }

    #[test]
    fn invalid_utf8_reports_an_error_and_converts_lossily() {
        let string = ValkeyString::test([b'a', 0xff]);

        assert!(matches!(
            string.try_as_str(),
            Err(ValkeyError::Str("Couldn't parse as UTF-8 string"))
        ));
        assert_eq!(string.to_string_lossy(), "a�");
    }

    #[test]
    fn parses_signed_integer_boundaries() {
        let minimum = ValkeyString::test(i64::MIN.to_string());
        let maximum = ValkeyString::test(i64::MAX.to_string());

        assert_eq!(
            minimum
                .parse_integer()
                .expect("minimum signed integer should parse"),
            i64::MIN
        );
        assert_eq!(
            maximum
                .parse_integer()
                .expect("maximum signed integer should parse"),
            i64::MAX
        );
    }

    #[test]
    fn rejects_invalid_signed_and_unsigned_integers() {
        let signed_overflow = ValkeyString::test("9223372036854775808");
        let negative_unsigned = ValkeyString::test("-1");
        let unsigned_overflow = ValkeyString::test("18446744073709551616");

        assert!(matches!(
            signed_overflow.parse_integer(),
            Err(ValkeyError::Str("Couldn't parse as integer"))
        ));
        assert!(matches!(
            negative_unsigned.parse_unsigned_integer(),
            Err(ValkeyError::Str("Couldn't parse as unsigned integer"))
        ));
        assert!(matches!(
            unsigned_overflow.parse_unsigned_integer(),
            Err(ValkeyError::Str("Couldn't parse as unsigned integer"))
        ));
    }

    #[test]
    fn rejects_invalid_float() {
        let string = ValkeyString::test("not-a-float");

        assert!(matches!(
            string.parse_float(),
            Err(ValkeyError::Str("Couldn't parse as float"))
        ));
    }

    #[test]
    fn next_arg_helpers_consume_values_of_each_supported_type() {
        let mut args = vec![
            ValkeyString::test([0, 0xff]),
            ValkeyString::test("string"),
            ValkeyString::test("text"),
            ValkeyString::test("-42"),
            ValkeyString::test(u64::MAX.to_string()),
            ValkeyString::test("42.5"),
        ]
        .into_iter();

        assert_eq!(
            args.next_arg()
                .expect("binary argument should be returned")
                .as_slice(),
            &[0, 0xff]
        );
        assert_eq!(
            args.next_string()
                .expect("string argument should be returned"),
            "string"
        );
        assert!(args.next_str().is_ok());
        assert_eq!(args.next_i64().expect("integer should parse"), -42);
        assert_eq!(
            args.next_u64().expect("unsigned integer should parse"),
            u64::MAX
        );
        assert_eq!(args.next_f64().expect("float should parse"), 42.5);
        assert!(args.done().is_ok());
    }

    #[test]
    fn next_arg_helpers_report_wrong_arity_for_empty_iterator() {
        let mut args = Vec::<ValkeyString>::new().into_iter();

        assert!(matches!(args.next_arg(), Err(ValkeyError::WrongArity)));
        assert!(matches!(args.next_string(), Err(ValkeyError::WrongArity)));
        assert!(matches!(args.next_str(), Err(ValkeyError::WrongArity)));
        assert!(matches!(args.next_i64(), Err(ValkeyError::WrongArity)));
        assert!(matches!(args.next_u64(), Err(ValkeyError::WrongArity)));
        assert!(matches!(args.next_f64(), Err(ValkeyError::WrongArity)));
    }

    #[test]
    fn next_str_rejects_invalid_utf8_while_next_string_is_lossy() {
        let mut string_args = vec![ValkeyString::test([b'a', 0xff])].into_iter();
        let mut str_args = vec![ValkeyString::test([b'a', 0xff])].into_iter();

        assert_eq!(
            string_args
                .next_string()
                .expect("next_string should use a lossy conversion"),
            "a�"
        );
        assert!(matches!(
            str_args.next_str(),
            Err(ValkeyError::Str("Couldn't parse as UTF-8 string"))
        ));
    }

    #[test]
    fn numeric_next_arg_helpers_report_parse_errors() {
        let mut integer_args = vec![ValkeyString::test("not-an-integer")].into_iter();
        let mut unsigned_args = vec![ValkeyString::test("-1")].into_iter();
        let mut float_args = vec![ValkeyString::test("not-a-float")].into_iter();

        assert!(matches!(
            integer_args.next_i64(),
            Err(ValkeyError::Str("Couldn't parse as integer"))
        ));
        assert!(matches!(
            unsigned_args.next_u64(),
            Err(ValkeyError::Str("Couldn't parse as unsigned integer"))
        ));
        assert!(matches!(
            float_args.next_f64(),
            Err(ValkeyError::Str("Couldn't parse as float"))
        ));
    }

    #[test]
    fn done_rejects_remaining_arguments() {
        let mut args = vec![ValkeyString::test("extra")].into_iter();

        assert!(matches!(args.done(), Err(ValkeyError::WrongArity)));
    }
}
