use dashmap::DashMap;
use std::os::raw::c_int;
use std::sync::LazyLock;
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::server_events::ClientChangeSubevent;
use valkey_module::{
    valkey_module, CommandFilterCtx, Context, RedisModuleCommandFilterCtx, Status, ValkeyError,
    ValkeyString, AUTH_HANDLED, AUTH_NOT_HANDLED, VALKEYMODULE_CMDFILTER_NOSELF,
};
use valkey_module_macros::client_changed_event_handler;

// filters do not have Context so we cannot access username in the filter directly
// mapping client_id to username and then lookup username by client_id which is available via CommandFilterCtx

// DashMap is a concurrent, thread-safe replacement for HashMap
// it allows multiple readers and writers with no locking required on reads, and fine-grained locks for writes.
static CLIENT_ID_USERNAME_MAP: LazyLock<DashMap<u64, String>> = LazyLock::new(|| DashMap::new());

// fires on every client connect and disconnect event
#[client_changed_event_handler]
fn client_changed_event_handler(ctx: &Context, client_event: ClientChangeSubevent) {
    match client_event {
        ClientChangeSubevent::Connected => {
            // user has not authed yet and might not so set username as default
            let username = "default".to_string();
            let client_id = ctx.get_client_id();
            CLIENT_ID_USERNAME_MAP.insert(client_id, username);
        }
        ClientChangeSubevent::Disconnected => {
            // remove the client_id from the map
            let client_id = ctx.get_client_id();
            CLIENT_ID_USERNAME_MAP.remove(&client_id);
        }
    }
}

// fires after client has authenticated so we know the new username
fn auth_callback(
    ctx: &Context,
    username: ValkeyString,
    _password: ValkeyString,
) -> Result<c_int, ValkeyError> {
    // if needed, we can get the previous username
    let _username_before_auth = match ctx.get_client_username() {
        Ok(tmp) => tmp.to_string(),
        Err(_err) => "default".to_string(),
    };
    if ctx.authenticate_client_with_acl_user(&username) == Status::Ok {
        let client_id = ctx.get_client_id();
        // map client_id to username
        CLIENT_ID_USERNAME_MAP.insert(client_id, username.to_string());
        return Ok(AUTH_HANDLED);
    }
    Ok(AUTH_NOT_HANDLED)
}

fn filter1_fn(ctx: *mut RedisModuleCommandFilterCtx) {
    // registered via valkey_module! macro
    // making sure that two modules can have the same filter fn name
    let cf_ctx = CommandFilterCtx::new(ctx);
    let client_id = cf_ctx.get_client_id();
    // lookup username by client_id
    let _username = match CLIENT_ID_USERNAME_MAP.get(&client_id) {
        Some(tmp) => tmp.clone(),
        None => "default".to_string(),
    };
    // do something with the username
}

fn filter2_fn(_ctx: *mut RedisModuleCommandFilterCtx) {
    // do something here, registered via valkey_module! macro
    // making sure that two modules can have the same filter fn name
}

valkey_module! {
    name: "filter2",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    auth: [auth_callback],
    commands: [
    ],
    filters: [
        [filter1_fn, VALKEYMODULE_CMDFILTER_NOSELF],
        [filter2_fn, VALKEYMODULE_CMDFILTER_NOSELF]
    ]
}
