use std::sync::{LazyLock, Mutex};
use valkey_module::alloc::ValkeyAlloc;

use valkey_module::{valkey_module, Context, ValkeyResult, ValkeyString, ValkeyValue};

static LAST_AUTH_USER: LazyLock<Mutex<(String, String)>> = LazyLock::new(|| Mutex::new((String::new(), String::new())));

// This callback will be scheduled to run after the auth handler completes.
// It receives retained `ValkeyString` previous and new usernames.
fn post_auth_callback(ctx: &Context, prev_user: ValkeyString, new_user: ValkeyString) {
    ctx.log_notice(&format!("post_auth: prev_user='{}', new_user='{}'", prev_user, new_user));
    let mut lock = LAST_AUTH_USER.lock().unwrap();
    *lock = (prev_user.to_string_lossy(), new_user.to_string_lossy());
}

fn whoami(_ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    let (prev, new) = LAST_AUTH_USER.lock().unwrap().clone();
    if new.is_empty() {
        Ok(ValkeyValue::Null)
    } else {
        Ok(ValkeyValue::Array(vec![ValkeyValue::BulkString(prev), ValkeyValue::BulkString(new)]))
    }
}

valkey_module! {
    name: "post_auth_example",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    post_auth: [ post_auth_callback ],
    commands: [
        ["whoami_postauth", whoami, "readonly", 0, 0, 0],
    ]
}
