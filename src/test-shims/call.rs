use crate::redisvalue::ValkeyValueKey;
use crate::{raw, ValkeyValue};
use std::sync::Arc;

pub(super) fn install() {
    // SAFETY: `setup_test_shims` calls this once after verifying the real API is uninitialized.
    unsafe {
        raw::RedisModule_CallReplyType = Some(call_reply_type);
        raw::RedisModule_FreeCallReply = Some(free_call_reply);
        raw::RedisModule_CallReplyInteger = Some(call_reply_integer);
        raw::RedisModule_CallReplyBool = Some(call_reply_bool);
        raw::RedisModule_CallReplyDouble = Some(call_reply_double);
        raw::RedisModule_CallReplyBigNumber = Some(call_reply_big_number);
        raw::RedisModule_CallReplyLength = Some(call_reply_length);
        raw::RedisModule_CallReplyArrayElement = Some(call_reply_array_element);
        raw::RedisModule_CallReplyMapElement = Some(call_reply_map_element);
        raw::RedisModule_CallReplyStringPtr = Some(call_reply_string_ptr);
    }
}

/// Owns a test-shim call reply while allowing array elements to share its value.
#[derive(Clone)]
pub(super) struct TestCallReply {
    value: Arc<TestCallReplyValue>,
}

/// Stores the reply variants exposed through Valkey's call-reply API.
enum TestCallReplyValue {
    String(Vec<u8>),
    Error(Vec<u8>),
    Integer(i64),
    Bool(bool),
    Double(f64),
    BigNumber(Vec<u8>),
    Array(Vec<Arc<TestCallReplyValue>>),
    Map(Vec<(Arc<TestCallReplyValue>, Arc<TestCallReplyValue>)>),
    Null,
}

// Constructs and transfers owned mock call-reply handles.
impl TestCallReply {
    /// Converts a supported high-level value into a mock call reply.
    pub(super) fn from_value(value: ValkeyValue) -> Result<Self, &'static str> {
        Ok(Self {
            value: TestCallReplyValue::from_value(value)?,
        })
    }

    /// Creates an error reply for an unexpected call.
    pub(super) fn error(message: impl Into<Vec<u8>>) -> Self {
        Self {
            value: Arc::new(TestCallReplyValue::Error(message.into())),
        }
    }

    /// Transfers this reply to the raw API; `free_call_reply` reclaims it.
    pub(super) fn into_raw(self) -> *mut raw::RedisModuleCallReply {
        Box::into_raw(Box::new(self)).cast()
    }
}

// Converts high-level test values into the reply variants supported by the shim.
impl TestCallReplyValue {
    /// Recursively converts values that the call-reply shim can expose.
    fn from_value(value: ValkeyValue) -> Result<Arc<Self>, &'static str> {
        let value = match value {
            ValkeyValue::SimpleString(value) => Self::String(value.into_bytes()),
            ValkeyValue::SimpleStringStatic(value) => Self::String(value.as_bytes().to_vec()),
            ValkeyValue::BulkString(value) => Self::String(value.into_bytes()),
            ValkeyValue::StringBuffer(value) => Self::String(value),
            ValkeyValue::Integer(value) => Self::Integer(value),
            ValkeyValue::Bool(value) => Self::Bool(value),
            ValkeyValue::Float(value) => Self::Double(value),
            ValkeyValue::BigNumber(value) => Self::BigNumber(value.into_bytes()),
            ValkeyValue::Array(values) => Self::Array(
                values
                    .into_iter()
                    .map(Self::from_value)
                    .collect::<Result<_, _>>()?,
            ),
            ValkeyValue::Map(values) => Self::Map(Self::map_entries(values)?),
            ValkeyValue::OrderedMap(values) => Self::Map(Self::map_entries(values)?),
            ValkeyValue::Null => Self::Null,
            ValkeyValue::StaticError(value) => Self::Error(value.as_bytes().to_vec()),
            _ => return Err("test-shim calls do not support this reply type"),
        };
        Ok(Arc::new(value))
    }

    /// Converts each map key and recursively converts its associated value.
    fn map_entries(
        values: impl IntoIterator<Item = (ValkeyValueKey, ValkeyValue)>,
    ) -> Result<Vec<(Arc<Self>, Arc<Self>)>, &'static str> {
        values
            .into_iter()
            .map(|(key, value)| Ok((Self::from_key(key), Self::from_value(value)?)))
            .collect()
    }

    /// Converts a `ValkeyValueKey` into a reply value that can be returned by map iteration.
    fn from_key(value: ValkeyValueKey) -> Arc<Self> {
        Arc::new(match value {
            ValkeyValueKey::Integer(value) => Self::Integer(value),
            ValkeyValueKey::String(value) => Self::String(value.into_bytes()),
            ValkeyValueKey::BulkValkeyString(value) => Self::String(value.as_slice().to_vec()),
            ValkeyValueKey::BulkString(value) => Self::String(value),
            ValkeyValueKey::Bool(value) => Self::Bool(value),
        })
    }
}

/// Implements `RedisModule_CallReplyType` for test replies.
pub(super) extern "C" fn call_reply_type(reply: *mut raw::RedisModuleCallReply) -> libc::c_int {
    with_reply_value(reply, |value| match value {
        TestCallReplyValue::String(_) => raw::ReplyType::String as libc::c_int,
        TestCallReplyValue::Error(_) => raw::ReplyType::Error as libc::c_int,
        TestCallReplyValue::Integer(_) => raw::ReplyType::Integer as libc::c_int,
        TestCallReplyValue::Bool(_) => raw::ReplyType::Bool as libc::c_int,
        TestCallReplyValue::Double(_) => raw::ReplyType::Double as libc::c_int,
        TestCallReplyValue::BigNumber(_) => raw::ReplyType::BigNumber as libc::c_int,
        TestCallReplyValue::Array(_) => raw::ReplyType::Array as libc::c_int,
        TestCallReplyValue::Map(_) => raw::ReplyType::Map as libc::c_int,
        TestCallReplyValue::Null => raw::ReplyType::Null as libc::c_int,
    })
    .unwrap_or(raw::ReplyType::Null as libc::c_int)
}

/// Implements `RedisModule_FreeCallReply` for test replies.
pub(super) extern "C" fn free_call_reply(reply: *mut raw::RedisModuleCallReply) {
    if !reply.is_null() {
        // SAFETY: each reply pointer is allocated as a `TestCallReply` handle.
        unsafe { drop(Box::from_raw(reply.cast::<TestCallReply>())) };
    }
}

/// Implements `RedisModule_CallReplyInteger` for test replies.
pub(super) extern "C" fn call_reply_integer(
    reply: *mut raw::RedisModuleCallReply,
) -> libc::c_longlong {
    with_reply_value(reply, |value| match value {
        TestCallReplyValue::Integer(value) => *value,
        _ => 0,
    })
    .unwrap_or(0)
}

/// Implements `RedisModule_CallReplyBool` for test replies.
pub(super) extern "C" fn call_reply_bool(reply: *mut raw::RedisModuleCallReply) -> libc::c_int {
    with_reply_value(reply, |value| {
        matches!(value, TestCallReplyValue::Bool(true))
    })
    .unwrap_or(false) as libc::c_int
}

/// Implements `RedisModule_CallReplyDouble` for test replies.
pub(super) extern "C" fn call_reply_double(reply: *mut raw::RedisModuleCallReply) -> f64 {
    with_reply_value(reply, |value| match value {
        TestCallReplyValue::Double(value) => *value,
        _ => 0.0,
    })
    .unwrap_or(0.0)
}

/// Implements `RedisModule_CallReplyBigNumber` for test replies.
pub(super) extern "C" fn call_reply_big_number(
    reply: *mut raw::RedisModuleCallReply,
    len: *mut usize,
) -> *const libc::c_char {
    with_reply_value(reply, |value| {
        let TestCallReplyValue::BigNumber(value) = value else {
            return std::ptr::null();
        };
        if !len.is_null() {
            // SAFETY: Valkey supplies a writable length output pointer.
            unsafe { len.write(value.len()) };
        }
        value.as_ptr().cast()
    })
    .unwrap_or(std::ptr::null())
}

/// Implements `RedisModule_CallReplyLength` for test replies.
pub(super) extern "C" fn call_reply_length(reply: *mut raw::RedisModuleCallReply) -> usize {
    with_reply_value(reply, |value| match value {
        TestCallReplyValue::Array(values) => values.len(),
        TestCallReplyValue::Map(values) => values.len(),
        _ => 0,
    })
    .unwrap_or(0)
}

/// Implements `RedisModule_CallReplyArrayElement` for test replies.
pub(super) extern "C" fn call_reply_array_element(
    reply: *mut raw::RedisModuleCallReply,
    index: usize,
) -> *mut raw::RedisModuleCallReply {
    with_reply_value(reply, |value| {
        let TestCallReplyValue::Array(values) = value else {
            return std::ptr::null_mut();
        };
        let Some(value) = values.get(index) else {
            return std::ptr::null_mut();
        };
        TestCallReply {
            value: Arc::clone(value),
        }
        .into_raw()
    })
    .unwrap_or(std::ptr::null_mut())
}

/// Implements `RedisModule_CallReplyMapElement` for test replies.
pub(super) extern "C" fn call_reply_map_element(
    reply: *mut raw::RedisModuleCallReply,
    index: usize,
    key: *mut *mut raw::RedisModuleCallReply,
    value: *mut *mut raw::RedisModuleCallReply,
) -> libc::c_int {
    with_reply_value(reply, |reply| {
        let TestCallReplyValue::Map(entries) = reply else {
            return raw::Status::Err as libc::c_int;
        };
        let Some((map_key, map_value)) = entries.get(index) else {
            return raw::Status::Err as libc::c_int;
        };
        if key.is_null() || value.is_null() {
            return raw::Status::Err as libc::c_int;
        }

        // SAFETY: both output pointers are non-null and receive newly allocated reply handles.
        unsafe {
            key.write(
                TestCallReply {
                    value: Arc::clone(map_key),
                }
                .into_raw(),
            );
            value.write(
                TestCallReply {
                    value: Arc::clone(map_value),
                }
                .into_raw(),
            );
        }
        raw::Status::Ok as libc::c_int
    })
    .unwrap_or(raw::Status::Err as libc::c_int)
}

/// Implements `RedisModule_CallReplyStringPtr` for test replies.
pub(super) extern "C" fn call_reply_string_ptr(
    reply: *mut raw::RedisModuleCallReply,
    len: *mut usize,
) -> *const libc::c_char {
    with_reply_value(reply, |value| {
        let value = match value {
            TestCallReplyValue::String(value) | TestCallReplyValue::Error(value) => value,
            _ => return std::ptr::null(),
        };
        if !len.is_null() {
            // SAFETY: Valkey supplies a writable length output pointer.
            unsafe { len.write(value.len()) };
        }
        value.as_ptr().cast()
    })
    .unwrap_or(std::ptr::null())
}

/// Calls `f` with the value held by a non-null test reply.
fn with_reply_value<R>(
    reply: *mut raw::RedisModuleCallReply,
    f: impl FnOnce(&TestCallReplyValue) -> R,
) -> Option<R> {
    if reply.is_null() {
        return None;
    }

    // SAFETY: all non-null replies originate from `TestCallReply::into_raw` or
    // `call_reply_array_element`, which allocate a `TestCallReply` handle.
    // SAFETY: the handle remains live for the duration of this callback.
    Some(f(unsafe { &*reply.cast::<TestCallReply>() }.value.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redisvalue::ValkeyValueKey;
    use std::collections::HashMap;

    #[test]
    fn exposes_scalar_reply_values() {
        let string = raw_reply(ValkeyValue::StringBuffer(vec![0, 0xff]));
        assert_eq!(
            call_reply_type(string),
            raw::ReplyType::String as libc::c_int
        );
        assert_eq!(
            unsafe { reply_bytes(string, call_reply_string_ptr) },
            [0, 0xff]
        );
        free_call_reply(string);

        let error = TestCallReply::error(b"ERR failure".to_vec()).into_raw();
        assert_eq!(call_reply_type(error), raw::ReplyType::Error as libc::c_int);
        assert_eq!(
            unsafe { reply_bytes(error, call_reply_string_ptr) },
            b"ERR failure"
        );
        free_call_reply(error);

        let integer = raw_reply(ValkeyValue::Integer(-42));
        assert_eq!(
            call_reply_type(integer),
            raw::ReplyType::Integer as libc::c_int
        );
        assert_eq!(call_reply_integer(integer), -42);
        free_call_reply(integer);

        let boolean = raw_reply(ValkeyValue::Bool(true));
        assert_eq!(
            call_reply_type(boolean),
            raw::ReplyType::Bool as libc::c_int
        );
        assert_eq!(call_reply_bool(boolean), 1);
        free_call_reply(boolean);

        let double = raw_reply(ValkeyValue::Float(1.5));
        assert_eq!(
            call_reply_type(double),
            raw::ReplyType::Double as libc::c_int
        );
        assert_eq!(call_reply_double(double), 1.5);
        free_call_reply(double);

        let big_number = raw_reply(ValkeyValue::BigNumber("12345678901234567890".to_owned()));
        assert_eq!(
            call_reply_type(big_number),
            raw::ReplyType::BigNumber as libc::c_int
        );
        assert_eq!(
            unsafe { reply_bytes(big_number, call_reply_big_number) },
            b"12345678901234567890"
        );
        free_call_reply(big_number);
    }

    #[test]
    fn exposes_nested_array_replies_and_nulls() {
        let reply = raw_reply(ValkeyValue::Array(vec![
            ValkeyValue::Integer(7),
            ValkeyValue::Array(vec![ValkeyValue::Null]),
        ]));

        assert_eq!(call_reply_type(reply), raw::ReplyType::Array as libc::c_int);
        assert_eq!(call_reply_length(reply), 2);
        assert!(call_reply_array_element(reply, 2).is_null());

        let integer = call_reply_array_element(reply, 0);
        assert_eq!(call_reply_integer(integer), 7);
        free_call_reply(integer);

        let nested = call_reply_array_element(reply, 1);
        assert_eq!(call_reply_length(nested), 1);
        let null = call_reply_array_element(nested, 0);
        assert_eq!(call_reply_type(null), raw::ReplyType::Null as libc::c_int);
        free_call_reply(null);
        free_call_reply(nested);
        free_call_reply(reply);
    }

    #[test]
    fn exposes_map_reply_entries() {
        let reply = raw_reply(ValkeyValue::Map(HashMap::from([(
            ValkeyValueKey::String("key".to_owned()),
            ValkeyValue::SimpleString("value".to_owned()),
        )])));

        assert_eq!(call_reply_type(reply), raw::ReplyType::Map as libc::c_int);
        assert_eq!(call_reply_length(reply), 1);

        let mut key = std::ptr::null_mut();
        let mut value = std::ptr::null_mut();
        assert_eq!(
            call_reply_map_element(reply, 0, &mut key, &mut value),
            raw::Status::Ok as libc::c_int
        );
        assert_eq!(unsafe { reply_bytes(key, call_reply_string_ptr) }, b"key");
        assert_eq!(
            unsafe { reply_bytes(value, call_reply_string_ptr) },
            b"value"
        );
        free_call_reply(key);
        free_call_reply(value);
        free_call_reply(reply);
    }

    #[test]
    fn rejects_unsupported_reply_type() {
        assert!(matches!(
            TestCallReply::from_value(ValkeyValue::Set(std::collections::HashSet::new())),
            Err("test-shim calls do not support this reply type")
        ));
    }

    fn raw_reply(value: ValkeyValue) -> *mut raw::RedisModuleCallReply {
        TestCallReply::from_value(value)
            .expect("reply value should be supported by the test shim")
            .into_raw()
    }

    unsafe fn reply_bytes(
        reply: *mut raw::RedisModuleCallReply,
        get: extern "C" fn(*mut raw::RedisModuleCallReply, *mut usize) -> *const libc::c_char,
    ) -> Vec<u8> {
        let mut len = 0;
        let value = get(reply, &mut len);
        // SAFETY: `get` returns a pointer into the live test reply, and `len` is its byte length.
        unsafe { std::slice::from_raw_parts(value.cast::<u8>(), len).to_vec() }
    }
}
