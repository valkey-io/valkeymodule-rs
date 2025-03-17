//! Example authentication module demonstrating both non-blocking and blocking authentication flows.
//!
//! This module shows two authentication patterns:
//! 1. Non-blocking authentication (auth_callback_one, auth_callback_two)
//!    - Direct validation against credentials
//!    - Immediate response
//!
//! 2. Blocking authentication (blocking_auth_callback_one, blocking_auth_callback_two)
//!    - Asynchronous validation in separate threads
//!    - Uses block_client_on_auth API

use std::fmt;
use std::os::raw::c_int;
use std::thread::sleep;
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{
    valkey_module, Context, Status, ValkeyError, ValkeyString, AUTH_HANDLED, AUTH_NOT_HANDLED,
};

// Represents the result of an authentication attempt
#[derive(Debug, Copy, Clone)]
enum AuthResult {
    Allow, // Allow the authentication request - credentials are valid for this authentication module
    Deny, // Deny the authentication request - credentials are invalid for this authentication module
    Next, // Pass the authentication request to the next module in chain - this module cannot determine validity
}

// Private data structure used to pass authentication results between callbacks
#[derive(Debug)]
struct AuthPrivData {
    result: AuthResult,
}

// Implement Display for AuthResult
impl fmt::Display for AuthResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// Common authentication function that validates user credentials against expected values
//
// # Arguments
// * `ctx` - Valkey module context
// * `username` - Username to validate
// * `password` - Password to validate
// * `expected_user` - Expected username
// * `expected_pass` - Expected password
// * `callback_name` - Name of the callback (for logging)
fn authenticate_user_common(
    ctx: &Context,
    username: &ValkeyString,
    password: &ValkeyString,
    expected_user: &str,
    expected_pass: &str,
    callback_name: &str,
) -> Result<c_int, ValkeyError> {
    ctx.log_notice(&format!(
        "{} auth attempt for user: {}",
        callback_name, username
    ));

    // If username matches our expected user
    if username.to_string() == expected_user {
        // If password also matches - try to authenticate
        if password.to_string() == expected_pass {
            ctx.log_notice(&format!(
                "{}: Matched user: {} with credentials",
                callback_name, username
            ));
            match ctx.authenticate_client_with_acl_user(username) {
                Status::Ok => {
                    ctx.log_notice(&format!(
                        "{}: Successfully authenticated user: {}",
                        callback_name, username
                    ));
                    Ok(AUTH_HANDLED)
                }
                Status::Err => {
                    ctx.log_warning(&format!(
                        "{}: Failed to authenticate user: {} with ACL",
                        callback_name, username
                    ));
                    Err(ValkeyError::Str("Failed to authenticate with ACL"))
                }
            }
        } else {
            // Username matches but password is wrong - explicit deny
            ctx.log_notice(&format!(
                "{}: Auth explicitly denied for user: {}",
                callback_name, username
            ));
            if callback_name.contains("auth_one") {
                Err(ValkeyError::Str(
                    "DENIED: Authentication credentials mismatch in auth_one",
                ))
            } else if callback_name.contains("auth_two") {
                Err(ValkeyError::Str(
                    "DENIED: Authentication credentials mismatch in auth_two",
                ))
            } else {
                Err(ValkeyError::Str(
                    "DENIED: Authentication credentials mismatch for the user",
                ))
            }
        }
    } else {
        // Username doesn't match - pass to next handler
        ctx.log_notice(&format!(
            "{}: auth not handled for user: {}",
            callback_name, username
        ));
        Ok(AUTH_NOT_HANDLED)
    }
}

// Authentication callback registered with Valkey via VM_RegisterAuthCallback
//
// This callback is registered in the authentication chain and processes authentication
// requests for user1. It is registered as the first callback in the LIFO chain in this example.
//
// # Authentication Rules
// Validates against:
// - Username: "user1"
// - Password: "module_pass1"
//
// # Arguments
// * `ctx` - Valkey module context
// * `username` - Username from client authentication attempt
// * `password` - Password from client authentication attempt
//
// # Returns
// * `Ok(AUTH_HANDLED)` - If authentication succeeds/fails for user1
// * `Ok(AUTH_NOT_HANDLED)` - If credentials don't match user1 pattern
// * `Err(ValkeyError)` - Provides the error response of authentication
fn auth_callback_one(
    ctx: &Context,
    username: ValkeyString,
    password: ValkeyString,
) -> Result<c_int, ValkeyError> {
    authenticate_user_common(
        ctx,
        &username,
        &password,
        "user1",
        "module_pass1",
        "auth_one",
    )
}

// Authentication callback for user2
//
// Similar to `auth_callback_one` but handles user2/module_pass2 credentials.
// Registered as the second callback in the LIFO chain.
// See `auth_callback_one` for detailed documentation.
//
// # Authentication Rules
// Validates against:
// - Username: "user2"
// - Password: "module_pass2"
fn auth_callback_two(
    ctx: &Context,
    username: ValkeyString,
    password: ValkeyString,
) -> Result<c_int, ValkeyError> {
    authenticate_user_common(
        ctx,
        &username,
        &password,
        "user2",
        "module_pass2",
        "auth_two",
    )
}

// Common function for authentication reply callback that processes authentication results after client was unblocked
//
// # Arguments
// * `ctx` - Valkey module context
// * `username` - Username being authenticated
// * `_password` - Password (unused in reply)
// * `priv_data` - Optional private data for authentication
// * `callback_name` - Name of the callback (for logging)
fn auth_reply_callback_common(
    ctx: &Context,
    username: ValkeyString,
    _password: ValkeyString,
    priv_data: Option<&AuthPrivData>,
    callback_name: &str,
) -> Result<c_int, ValkeyError> {
    match priv_data
        .map(|data| data.result)
        .unwrap_or(AuthResult::Next)
    {
        AuthResult::Allow => {
            ctx.log_notice(&format!(
                "{}: Auth allowed for user: {}",
                callback_name, username
            ));
            match ctx.authenticate_client_with_acl_user(&username) {
                Status::Ok => {
                    ctx.log_notice(&format!(
                        "{}: Successfully authenticated user: {}",
                        callback_name, username
                    ));
                    Ok(AUTH_HANDLED)
                }
                Status::Err => {
                    ctx.log_warning(&format!(
                        "{}: Failed to authenticate user: {} with ACL",
                        callback_name, username
                    ));
                    Err(ValkeyError::Str("Failed to authenticate with ACL"))
                }
            }
        }
        AuthResult::Deny => {
            ctx.log_notice(&format!(
                "{}: Auth explicitly denied for user: {}",
                callback_name, username
            ));
            if callback_name.contains("blocked_auth_reply_one") {
                Err(ValkeyError::Str(
                    "DENIED: Authentication credentials mismatch in blocked_auth_reply_one",
                ))
            } else if callback_name.contains("blocked_auth_reply_two") {
                Err(ValkeyError::Str(
                    "DENIED: Authentication credentials mismatch in blocked_auth_reply_two",
                ))
            } else {
                Err(ValkeyError::Str(
                    "DENIED: Authentication credentials mismatch for the user",
                ))
            }
        }
        AuthResult::Next => {
            ctx.log_notice(&format!(
                "{}: Passing auth to next handler for user: {}",
                callback_name, username
            ));
            Ok(AUTH_NOT_HANDLED)
        }
    }
}

// Reply callback for blocked clients used in blocking_auth_callback_one
//
// This callback is registered through ValkeyModule_BlockClientOnAuth API in blocking_auth_callback_one.
// When the blocked client is unblocked using ValkeyModule_UnblockClient, the Valkey core will
// automatically invoke this registered reply callback to process the authentication result.
//
// # Arguments
//
// * `ctx` - Valkey module context for executing commands and accessing Valkey functionality
// * `username` - Username provided during authentication attempt
// * `password` - Password provided during authentication attempt
// * `priv_data` - Private data associated with the authentication context (Optional)
//
// # Returns
// * `Result<c_int, ValkeyError>` - Returns OK (0) on success, or error code on failure
fn auth_reply_callback_one(
    ctx: &Context,
    username: ValkeyString,
    password: ValkeyString,
    priv_data: Option<&AuthPrivData>,
) -> Result<c_int, ValkeyError> {
    auth_reply_callback_common(ctx, username, password, priv_data, "blocked_auth_reply_one")
}

// Reply callback for blocked clients used in blocking_auth_callback_two
//
// Similar to `auth_reply_callback_one` but handles replies for blocking_auth_callback_two.
// See `auth_reply_callback_one` for detailed documentation.
//
// # Returns
// * `Result<c_int, ValkeyError>` - Returns OK (0) on success, or error code on failure
fn auth_reply_callback_two(
    ctx: &Context,
    username: ValkeyString,
    password: ValkeyString,
    priv_data: Option<&AuthPrivData>,
) -> Result<c_int, ValkeyError> {
    auth_reply_callback_common(ctx, username, password, priv_data, "blocked_auth_reply_two")
}

// Cleanup callback for private data in blocking_auth_callback_one
//
// This callback is registered through ValkeyModule_BlockClientOnAuth API along with the reply callback.
// It is automatically invoked by Valkey core after the reply callback completes, following this sequence:
// unblock client -> reply callback -> free privdata callback.
//
// # Important Note
// This is an optional callback. If private data is allocated using ValkeyModule_Alloc or
// ValkeyModule_Realloc, users must ensure they register this callback to properly free the
// allocated memory to prevent memory leaks.
//
// # Arguments
//
// * `ctx` - Valkey module context for executing commands and logging
// * `data` - Private authentication data to be cleaned up
fn free_privdata_callback_one(ctx: &Context, data: AuthPrivData) {
    // Handle cleanup with typed data
    ctx.log_notice(&format!(
        "free_privdata_callback_one: Cleaning up: {}",
        data.result
    ));
    drop(data);
}

// Cleanup callback for private data in blocking_auth_callback_two
//
// Handles cleanup of authentication data for blockUser4-6.
// Similar to `free_privdata_callback_one` but handles cleanup for different users.
//
// # Arguments
// * `ctx` - Valkey module context for executing commands and logging
// * `data` - Private authentication data to be cleaned up
fn free_privdata_callback_two(ctx: &Context, data: AuthPrivData) {
    // Handle cleanup with typed data
    ctx.log_notice(&format!(
        "free_privdata_callback_two: Cleaning up: {}",
        data.result
    ));
    drop(data);
}

// Common implementation for blocking authentication handlers
//
// # Arguments
// * `ctx` - Valkey module context
// * `username` - Username to authenticate
// * `password` - Password to validate
// * `auth_reply_fn` - Function to handle authentication reply
// * `free_callback_fn` - Optional cleanup callback
// * `auth_patterns` - Function that defines authentication patterns and rules
// * `callback_name` - Name of the callback (for logging)
fn blocking_auth_common(
    ctx: &Context,
    username: ValkeyString,
    password: ValkeyString,
    auth_reply_fn: fn(
        &Context,
        ValkeyString,
        ValkeyString,
        Option<&AuthPrivData>,
    ) -> Result<c_int, ValkeyError>,
    free_callback_fn: Option<fn(&Context, AuthPrivData)>,
    auth_patterns: impl Fn(&str, &str) -> AuthResult + Send + 'static,
    callback_name: &str,
) -> Result<c_int, ValkeyError> {
    ctx.log_notice(&format!("{}: handling blocked client", callback_name));

    if username.to_string() == "default" {
        ctx.log_notice(&format!(
            "{}: Default user authentication - passing to next handler",
            callback_name
        ));
        return Ok(AUTH_NOT_HANDLED);
    }

    let username_str = username.to_string();
    let password_str = password.to_string();
    let callback_name_str = callback_name.to_string();

    let mut blocked_client = ctx.block_client_on_auth(auth_reply_fn, free_callback_fn);

    if callback_name_str == "blocking_auth_callback_two" {
        if username_str == "blockAbort" && password_str == "abort" {
            blocked_client.abort().unwrap_or_else(|e| {
                ctx.log_notice(&format!("Failed to abort blocked client: {:?}", e));
            });
            ctx.reply_error_string("ERR ABORT: Authentication aborted by server");
            return Ok(AUTH_HANDLED);
        }
    }

    // Normal authentication flow in thread
    std::thread::spawn(move || {
        // This check specifically looks for the blockUserDelay user in the second callback
        if callback_name_str == "blocking_auth_callback_two" && username_str == "blockUserDelay" {
            sleep(std::time::Duration::from_secs(2));
        }

        let result = auth_patterns(&username_str, &password_str);
        blocked_client.set_blocked_private_data(AuthPrivData { result });
    });

    Ok(AUTH_HANDLED)
}

// Authentication callback registered with Valkey via VM_RegisterAuthCallback
//
// This blocking handler processes authentication requests asynchronously for blockUser1-3.
// It is registered as the third callback in the LIFO chain in this example.
//
// # Authentication Rules
// - Allowed combinations:
//   * username: "blockUser1", password: "module_blockPass1"
//   * username: "blockUser2", password: "module_blockPass2"
//   * username: "blockUser3", password: "module_blockPass3"
// - Any other password for blockUser1/blockUser2/blockUser3: Denied
// - All other usernames: Passed to next handler in chain
//
// # Arguments
// * `ctx` - Valkey module context
// * `username` - Username from client authentication attempt
// * `password` - Password from client authentication attempt
//
// # Returns
// * `Ok(AUTH_HANDLED)` - Authentication was processed asynchronously
// * `Err(ValkeyError)` - Provides the error response of authentication
fn blocking_auth_callback_one(
    ctx: &Context,
    username: ValkeyString,
    password: ValkeyString,
) -> Result<c_int, ValkeyError> {
    blocking_auth_common(
        ctx,
        username,
        password,
        auth_reply_callback_one,
        Some(free_privdata_callback_one),
        |user, pass| match (user, pass) {
            ("blockUser1", "module_blockPass1") => AuthResult::Allow,
            ("blockUser2", "module_blockPass2") => AuthResult::Allow,
            ("blockUser3", "module_blockPass3") => AuthResult::Allow,
            ("blockUser1", _) | ("blockUser2", _) | ("blockUser3", _) => AuthResult::Deny,
            _ => AuthResult::Next,
        },
        "blocking_auth_callback_one",
    )
}

// Authentication callback for blockUser4-6
//
// Similar to blocking_auth_callback_one but handles a different set of users.
// Registered as the fourth (last) callback in the LIFO chain.
//
// # Authentication Rules
// - Allowed combinations:
//   * username: "blockUser4", password: "module_blockPass4"
//   * username: "blockUser5", password: "module_blockPass5"
//   * username: "blockUser6", password: "blockPass6"
//     (Special test user that simulates a 2-second authentication delay)
//   * username: "blockUserDelay", password: "blockPassDelay"
// - Any other password for these users: Denied
// - All other usernames: Passed to next handler
fn blocking_auth_callback_two(
    ctx: &Context,
    username: ValkeyString,
    password: ValkeyString,
) -> Result<c_int, ValkeyError> {
    blocking_auth_common(
        ctx,
        username,
        password,
        auth_reply_callback_two,
        Some(free_privdata_callback_two),
        |user, pass| match (user, pass) {
            ("blockUser4", "module_blockPass4") => AuthResult::Allow,
            ("blockUser5", "module_blockPass5") => AuthResult::Allow,
            ("blockUser6", "module_blockPass6") => AuthResult::Allow,
            ("blockUserDelay", "blockPassDelay") => AuthResult::Allow,
            ("blockUser4", _) | ("blockUser5", _) | ("blockUser6", _) | ("blockUserDelay", _) => {
                AuthResult::Deny
            }
            _ => AuthResult::Next,
        },
        "blocking_auth_callback_two",
    )
}

//////////////////////////////////////////////////////

// Valkey module declaration for authentication
// Registers authentication callbacks in LIFO (Last In, First Out) order:
// 4. blocking_auth_callback_two
// 3. blocking_auth_callback_one
// 2. auth_callback_two
// 1. auth_callback_one
valkey_module! {
    name: "auth",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    auth: [
        blocking_auth_callback_two,
        blocking_auth_callback_one,
        auth_callback_two,
        auth_callback_one
    ],
    commands: []
}
