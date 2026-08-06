mod command_filter_ctx;
mod context;
mod valkey_string;

pub use command_filter_ctx::TestCommandFilterCtx;
pub use context::TestContext;

use crate::raw;
use std::ptr::addr_of;
use std::sync::Once;

static INIT: Once = Once::new();

fn setup_test_shims() {
    INIT.call_once(|| {
        assert!(
            !real_valkey_api_is_initialized(),
            "refusing to install test shims inside a running Valkey process"
        );

        unsafe {
            // ValkeyString
            raw::RedisModule_StringPtrLen = Some(valkey_string::string_ptr_len);
            raw::RedisModule_FreeString = Some(valkey_string::free_string);
            raw::RedisModule_RetainString = Some(valkey_string::retain_string);
            raw::RedisModule_StringToLongLong = Some(valkey_string::string_to_longlong);
            raw::RedisModule_StringToULongLong = Some(valkey_string::string_to_ulonglong);
            raw::RedisModule_StringToDouble = Some(valkey_string::string_to_double);
            raw::RedisModule_CreateString = Some(valkey_string::create_string);
            raw::RedisModule_CreateStringFromString =
                Some(valkey_string::create_string_from_string);
            raw::RedisModule_StringCompare = Some(valkey_string::string_compare);
            // Context
            raw::RedisModule_GetClientId = Some(context::get_client_id);
            raw::RedisModule_GetClientNameById = Some(context::get_client_name_by_id);
            raw::RedisModule_GetClientUserNameById = Some(context::get_client_username_by_id);
            raw::RedisModule_GetClientInfoById = Some(context::get_client_info_by_id);
            raw::RedisModule_GetCurrentUserName = Some(context::get_current_user_name);
            raw::RedisModule_DeauthenticateAndCloseClient =
                Some(context::deauthenticate_and_close_client);
            raw::RedisModule_SetModuleOptions = Some(context::set_module_options);
            raw::RedisModule_GetServerVersion = Some(context::get_server_version);
            raw::RedisModule_AuthenticateClientWithACLUser =
                Some(context::authenticate_client_with_acl_user);
            // CommandFilterCtx
            raw::RedisModule_CommandFilterArgsCount =
                Some(command_filter_ctx::command_filter_args_count);
            raw::RedisModule_CommandFilterArgGet = Some(command_filter_ctx::command_filter_arg_get);
            raw::RedisModule_CommandFilterArgReplace =
                Some(command_filter_ctx::command_filter_arg_replace);
            raw::RedisModule_CommandFilterArgInsert =
                Some(command_filter_ctx::command_filter_arg_insert);
            raw::RedisModule_CommandFilterArgDelete =
                Some(command_filter_ctx::command_filter_arg_delete);
            raw::RedisModule_CommandFilterGetClientId =
                Some(command_filter_ctx::command_filter_get_client_id);
        }
    });
}

fn real_valkey_api_is_initialized() -> bool {
    // SAFETY: These function pointers are initialized by the real module API setup.
    // Reading copies through raw pointers avoids borrowing the mutable statics.
    unsafe {
        addr_of!(raw::RedisModule_GetApi).read().is_some()
            || addr_of!(raw::ValkeyModule_GetApi).read().is_some()
    }
}
