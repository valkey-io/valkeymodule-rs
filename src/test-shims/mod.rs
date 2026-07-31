mod valkey_string;

use crate::raw;
use std::sync::Once;

static INIT: Once = Once::new();

fn setup_test_shims() {
    INIT.call_once(|| unsafe {
        raw::RedisModule_StringPtrLen = Some(valkey_string::string_ptr_len);
        raw::RedisModule_FreeString = Some(valkey_string::free_string);
        raw::RedisModule_RetainString = Some(valkey_string::retain_string);
        raw::RedisModule_StringToLongLong = Some(valkey_string::string_to_longlong);
        raw::RedisModule_StringToULongLong = Some(valkey_string::string_to_ulonglong);
        raw::RedisModule_StringToDouble = Some(valkey_string::string_to_double);
        raw::RedisModule_CreateStringFromString = Some(valkey_string::create_string_from_string);
        raw::RedisModule_StringCompare = Some(valkey_string::string_compare);
    });
}
