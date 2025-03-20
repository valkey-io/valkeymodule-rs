use std::fmt;
use std::os::raw::c_int;
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{
    valkey_module, Context, Status, ValkeyError, ValkeyString, AUTH_HANDLED, AUTH_NOT_HANDLED,
};

#[derive(Debug)]
enum AuthResult {
    Allow,
    Deny,
    Next,
}

#[derive(Debug)]
struct AuthPrivData {
    result: AuthResult,
}

// Implement Display for AuthResult
impl fmt::Display for AuthResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthResult::Allow => write!(f, "Allow"),
            AuthResult::Deny => write!(f, "Deny"),
            AuthResult::Next => write!(f, "Next"),
        }
    }
}

fn authenticate_user(
    ctx: &Context,
    username: &ValkeyString,
    password: &ValkeyString,
    expected_user: &str,
    expected_pass: &str,
    auth_type: &str,
) -> Result<c_int, ValkeyError> {
    ctx.log_debug(&format!(
        "{} auth attempt for user: {}",
        auth_type,
        username.to_string()
    ));

    if username.to_string() == expected_user && password.to_string() == expected_pass {
        ctx.log_debug(&format!("Matched {} user credentials", auth_type));
        match ctx.authenticate_client_with_acl_user(username) {
            Status::Ok => {
                ctx.log_debug(&format!("Successfully authenticated {} user", auth_type));
                Ok(AUTH_HANDLED)
            }
            Status::Err => {
                ctx.log_warning(&format!(
                    "Failed to authenticate {} user with ACL",
                    auth_type
                ));
                Ok(AUTH_HANDLED)
            }
        }
    } else {
        ctx.log_debug(&format!(
            "{} auth not handled for user: {}",
            auth_type,
            username.to_string()
        ));
        Ok(AUTH_NOT_HANDLED)
    }
}

// Simplified callbacks using common function
fn auth_callback(
    ctx: &Context,
    username: ValkeyString,
    password: ValkeyString,
) -> Result<c_int, ValkeyError> {
    authenticate_user(ctx, &username, &password, "foo", "allow", "Standard")
}

fn bar_auth_callback(
    ctx: &Context,
    username: ValkeyString,
    password: ValkeyString,
) -> Result<c_int, ValkeyError> {
    authenticate_user(ctx, &username, &password, "bar", "secret", "Bar")
}

fn admin_auth_callback(
    ctx: &Context,
    username: ValkeyString,
    password: ValkeyString,
) -> Result<c_int, ValkeyError> {
    authenticate_user(
        ctx,
        &username,
        &password,
        "admin",
        "superSecret123",
        "Admin",
    )
}

// Core auth reply logic
fn my_auth_reply(
    name: &str,
    ctx: &Context,
    username: ValkeyString,
    _password: ValkeyString,
) -> Result<c_int, ValkeyError> {
    match ctx.get_blocked_client_privdata::<AuthPrivData>() {
        Some(priv_data) => match priv_data.result {
            AuthResult::Allow => {
                ctx.log_debug(&format!(
                    "{}: Auth allowed for user: {}",
                    name,
                    username.to_string()
                ));
                match ctx.authenticate_client_with_acl_user(&username) {
                    Status::Ok => {
                        ctx.log_debug(&format!(
                            "{}: Successfully authenticated user: {}",
                            name,
                            username.to_string()
                        ));
                        Ok(AUTH_HANDLED)
                    }
                    Status::Err => {
                        ctx.log_warning(&format!(
                            "{}: Failed to authenticate user: {} with ACL",
                            name,
                            username.to_string()
                        ));
                        Ok(AUTH_HANDLED)
                    }
                }
            }
            AuthResult::Deny => {
                ctx.log_debug(&format!(
                    "{}: Auth explicitly denied for user: {}",
                    name,
                    username.to_string()
                ));
                Ok(AUTH_HANDLED)
            }
            AuthResult::Next => {
                ctx.log_debug(&format!(
                    "{}: Passing auth to next handler for user: {}",
                    name,
                    username.to_string()
                ));
                Ok(AUTH_NOT_HANDLED)
            }
        },
        None => {
            ctx.log_warning(&format!("{}: No private data found in auth reply", name));
            Ok(AUTH_NOT_HANDLED)
        }
    }
}

fn my_auth_reply_one(
    ctx: &Context,
    username: ValkeyString,
    password: ValkeyString,
) -> Result<c_int, ValkeyError> {
    my_auth_reply("auth_one", ctx, username, password)
}

fn my_auth_reply_two(
    ctx: &Context,
    username: ValkeyString,
    password: ValkeyString,
) -> Result<c_int, ValkeyError> {
    my_auth_reply("auth_two", ctx, username, password)
}

fn my_free_privdata_callback_one(ctx: &Context, data: AuthPrivData) {
    // Handle cleanup with typed data
    ctx.log_debug(&format!(
        "my_free_privdata_callback_one: Cleaning up: {}",
        data.result
    ));
    drop(data);
}

fn my_free_privdata_callback_two(ctx: &Context, data: AuthPrivData) {
    // Handle cleanup with typed data
    ctx.log_debug(&format!(
        "my_free_privdata_callback_two: Cleaning up: {}",
        data.result
    ));
    drop(data);
}

fn blocking_auth_common(
    ctx: &Context,
    username: ValkeyString,
    password: ValkeyString,
    callback_name: &str,
    auth_reply_fn: fn(&Context, ValkeyString, ValkeyString) -> Result<c_int, ValkeyError>,
    free_callback_fn: Option<fn(&Context, AuthPrivData)>,
    auth_patterns: impl Fn(&str, &str) -> AuthResult + Send + 'static,
) -> Result<c_int, ValkeyError> {
    ctx.log_debug(&format!("{}: handling blocked client", callback_name));

    if username.to_string() == "default" {
        ctx.log_debug(&format!(
            "{}: Default user authentication - passing to next handler",
            callback_name
        ));
        return Ok(AUTH_NOT_HANDLED);
    }

    let username_str = username.to_string();
    let password_str = password.to_string();

    let mut blocked_client = ctx.block_client_on_auth(auth_reply_fn, free_callback_fn);

    // Normal authentication flow in thread
    std::thread::spawn(move || {
        if username_str == "abort-test" {
            // Abort the client directly from the thread
            blocked_client.abort().unwrap_or_else(|e| {
                println!("Failed to abort blocked client: {:?}", e);
            });
            return;
        }

        let result = auth_patterns(&username_str, &password_str);
        blocked_client.set_blocked_private_data(AuthPrivData { result });
    });

    Ok(AUTH_HANDLED)
}

fn blocking_auth_callback_one(
    ctx: &Context,
    username: ValkeyString,
    password: ValkeyString,
) -> Result<c_int, ValkeyError> {
    blocking_auth_common(
        ctx,
        username,
        password,
        "blocking_auth_callback_one",
        my_auth_reply_one,
        Some(my_free_privdata_callback_one),
        |user, pass| match (user, pass) {
            ("abc", "allow") => AuthResult::Allow,
            ("def", "secret") => AuthResult::Allow,
            ("ghk", "superSecret123") => AuthResult::Allow,
            ("abc", _) | ("def", _) | ("ghk", _) => AuthResult::Deny,
            _ => AuthResult::Next,
        },
    )
}

fn blocking_auth_callback_two(
    ctx: &Context,
    username: ValkeyString,
    password: ValkeyString,
) -> Result<c_int, ValkeyError> {
    blocking_auth_common(
        ctx,
        username,
        password,
        "blocking_auth_callback_two",
        my_auth_reply_two,
        Some(my_free_privdata_callback_two),
        |user, pass| match (user, pass) {
            ("ref", "allow") => AuthResult::Allow,
            ("def", "secret") => AuthResult::Allow,
            ("puf", "superSecret123") => AuthResult::Allow,
            ("ref", _) | ("def", _) | ("puf", _) => AuthResult::Deny,
            _ => AuthResult::Next,
        },
    )
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "auth",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    auth: [
        blocking_auth_callback_two,
        blocking_auth_callback_one,
        auth_callback,
        bar_auth_callback,
        admin_auth_callback
    ],
    commands: []
}
