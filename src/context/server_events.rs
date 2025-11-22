use std::ffi::CStr;

use crate::{context::Context, ValkeyError};
use crate::{raw, InfoContext, ValkeyResult};
use linkme::distributed_slice;

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum ServerRole {
    Primary,
    Replica,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum LoadingSubevent {
    RdbStarted,
    AofStarted,
    ReplStarted,
    Ended,
    Failed,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum LoadingProgressSubevent {
    Rdb,
    Aof,
}

#[derive(Clone, Copy, Debug)]
pub struct LoadingProgress {
    pub subevent: LoadingProgressSubevent,
    pub hz: i32,
    pub progress: i32,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum FlushSubevent {
    Started,
    Ended,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum ModuleChangeSubevent {
    Loaded,
    Unloaded,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum ClientChangeSubevent {
    Connected,
    Disconnected,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum KeyChangeSubevent {
    Deleted,
    Expired,
    Evicted,
    Overwritten,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum PersistenceSubevent {
    RdbStart,
    AofStart,
    SyncRdbStart,
    SyncAofStart,
    Ended,
    Failed,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum MasterLinkChangeSubevent {
    Up,
    Down,
}

#[derive(Clone)]
pub enum ServerEventHandler {
    RoleChanged(fn(&Context, ServerRole)),
    Loading(fn(&Context, LoadingSubevent)),
    Flush(fn(&Context, FlushSubevent)),
    ModuleChange(fn(&Context, ModuleChangeSubevent)),
    ClientChange(fn(&Context, ClientChangeSubevent)),
    KeyChangeSubevent(fn(&Context, KeyChangeSubevent)),
    PersistenceSubevent(fn(&Context, PersistenceSubevent)),
    LoadingProgress(fn(&Context, LoadingProgress)),
    MaterLinkChangeSubevent(fn(&Context, MasterLinkChangeSubevent)),
}

#[distributed_slice()]
pub static ROLE_CHANGED_SERVER_EVENTS_LIST: [fn(&Context, ServerRole)] = [..];

#[distributed_slice()]
pub static LOADING_SERVER_EVENTS_LIST: [fn(&Context, LoadingSubevent)] = [..];

#[distributed_slice()]
pub static LOADING_PROGRESS_SERVER_EVENTS_LIST: [fn(&Context, LoadingProgress)] = [..];

#[distributed_slice()]
pub static FLUSH_SERVER_EVENTS_LIST: [fn(&Context, FlushSubevent)] = [..];

#[distributed_slice()]
pub static MODULE_CHANGED_SERVER_EVENTS_LIST: [fn(&Context, ModuleChangeSubevent)] = [..];

#[distributed_slice()]
pub static CONFIG_CHANGED_SERVER_EVENTS_LIST: [fn(&Context, &[&str])] = [..];

#[distributed_slice()]
pub static CRON_SERVER_EVENTS_LIST: [fn(&Context, u64)] = [..];

#[distributed_slice()]
pub static INFO_COMMAND_HANDLER_LIST: [fn(&InfoContext, bool) -> ValkeyResult<()>] = [..];

#[distributed_slice()]
pub static CLIENT_CHANGED_SERVER_EVENTS_LIST: [fn(&Context, ClientChangeSubevent)] = [..];

#[distributed_slice()]
pub static KEY_SERVER_EVENTS_LIST: [fn(&Context, KeyChangeSubevent)] = [..];

#[distributed_slice()]
pub static SHUTDOWN_SERVER_EVENT_LIST: [fn(&Context, u64)] = [..];

#[distributed_slice()]
pub static PERSISTENCE_SERVER_EVENTS_LIST: [fn(&Context, PersistenceSubevent)] = [..];

#[distributed_slice()]
pub static MASTER_LINK_CHANGE_SERVER_EVENTS_LIST: [fn(&Context, MasterLinkChangeSubevent)] = [..];

extern "C" fn cron_callback(
    ctx: *mut raw::RedisModuleCtx,
    _eid: raw::RedisModuleEvent,
    _subevent: u64,
    data: *mut ::std::os::raw::c_void,
) {
    let data: &raw::RedisModuleConfigChangeV1 =
        unsafe { &*(data as *mut raw::RedisModuleConfigChangeV1) };
    let ctx = Context::new(ctx);
    CRON_SERVER_EVENTS_LIST.iter().for_each(|callback| {
        callback(&ctx, data.version);
    });
}

extern "C" fn role_changed_callback(
    ctx: *mut raw::RedisModuleCtx,
    _eid: raw::RedisModuleEvent,
    subevent: u64,
    _data: *mut ::std::os::raw::c_void,
) {
    let new_role = if subevent == raw::REDISMODULE_EVENT_REPLROLECHANGED_NOW_MASTER {
        ServerRole::Primary
    } else {
        ServerRole::Replica
    };
    let ctx = Context::new(ctx);
    ROLE_CHANGED_SERVER_EVENTS_LIST.iter().for_each(|callback| {
        callback(&ctx, new_role);
    });
}

extern "C" fn loading_event_callback(
    ctx: *mut raw::RedisModuleCtx,
    _eid: raw::RedisModuleEvent,
    subevent: u64,
    _data: *mut ::std::os::raw::c_void,
) {
    let loading_sub_event = match subevent {
        raw::REDISMODULE_SUBEVENT_LOADING_RDB_START => LoadingSubevent::RdbStarted,
        raw::REDISMODULE_SUBEVENT_LOADING_REPL_START => LoadingSubevent::ReplStarted,
        raw::REDISMODULE_SUBEVENT_LOADING_AOF_START => LoadingSubevent::AofStarted,
        raw::REDISMODULE_SUBEVENT_LOADING_ENDED => LoadingSubevent::Ended,
        _ => LoadingSubevent::Failed,
    };
    let ctx = Context::new(ctx);
    LOADING_SERVER_EVENTS_LIST.iter().for_each(|callback| {
        callback(&ctx, loading_sub_event);
    });
}

extern "C" fn loading_progress_event_callback(
    ctx: *mut raw::RedisModuleCtx,
    _eid: raw::RedisModuleEvent,
    subevent: u64,
    data: *mut ::std::os::raw::c_void,
) {
    if data.is_null() {
        return;
    }

    // Cast to the generated raw binding for the loading progress struct
    let info: &raw::RedisModuleLoadingProgressInfo = unsafe {
        &*(data as *mut raw::RedisModuleLoadingProgressInfo)
    };

    let hz = info.hz as i32;
    let progress = info.progress as i32;

    let sub = match subevent {
        raw::REDISMODULE_SUBEVENT_LOADING_PROGRESS_RDB => LoadingProgressSubevent::Rdb,
        raw::REDISMODULE_SUBEVENT_LOADING_PROGRESS_AOF => LoadingProgressSubevent::Aof,
        _ => return,
    };

    let ctx = Context::new(ctx);
    let payload = LoadingProgress {
        subevent: sub,
        hz,
        progress,
    };

    LOADING_PROGRESS_SERVER_EVENTS_LIST.iter().for_each(|callback| {
        callback(&ctx, payload);
    });
}

extern "C" fn flush_event_callback(
    ctx: *mut raw::RedisModuleCtx,
    _eid: raw::RedisModuleEvent,
    subevent: u64,
    _data: *mut ::std::os::raw::c_void,
) {
    let flush_sub_event = if subevent == raw::REDISMODULE_SUBEVENT_FLUSHDB_START {
        FlushSubevent::Started
    } else {
        FlushSubevent::Ended
    };
    let ctx = Context::new(ctx);
    FLUSH_SERVER_EVENTS_LIST.iter().for_each(|callback| {
        callback(&ctx, flush_sub_event);
    });
}

extern "C" fn module_change_event_callback(
    ctx: *mut raw::RedisModuleCtx,
    _eid: raw::RedisModuleEvent,
    subevent: u64,
    _data: *mut ::std::os::raw::c_void,
) {
    let module_changed_sub_event = if subevent == raw::REDISMODULE_SUBEVENT_MODULE_LOADED {
        ModuleChangeSubevent::Loaded
    } else {
        ModuleChangeSubevent::Unloaded
    };
    let ctx = Context::new(ctx);
    MODULE_CHANGED_SERVER_EVENTS_LIST
        .iter()
        .for_each(|callback| {
            callback(&ctx, module_changed_sub_event);
        });
}

extern "C" fn client_change_event_callback(
    ctx: *mut raw::RedisModuleCtx,
    _eid: raw::RedisModuleEvent,
    subevent: u64,
    _data: *mut ::std::os::raw::c_void,
) {
    let client_change_sub_event = if subevent == raw::REDISMODULE_SUBEVENT_CLIENT_CHANGE_CONNECTED {
        ClientChangeSubevent::Connected
    } else {
        ClientChangeSubevent::Disconnected
    };
    let ctx = Context::new(ctx);
    CLIENT_CHANGED_SERVER_EVENTS_LIST
        .iter()
        .for_each(|callback| {
            callback(&ctx, client_change_sub_event);
        });
}

extern "C" fn key_event_callback(
    ctx: *mut raw::RedisModuleCtx,
    _eid: raw::RedisModuleEvent,
    subevent: u64,
    _data: *mut ::std::os::raw::c_void,
) {
    let key_change_sub_event = match subevent {
        raw::REDISMODULE_SUBEVENT_KEY_DELETED => KeyChangeSubevent::Deleted,
        raw::REDISMODULE_SUBEVENT_KEY_EXPIRED => KeyChangeSubevent::Expired,
        raw::REDISMODULE_SUBEVENT_KEY_EVICTED => KeyChangeSubevent::Evicted,
        raw::REDISMODULE_SUBEVENT_KEY_OVERWRITTEN => KeyChangeSubevent::Overwritten,
        _ => return,
    };
    let ctx = Context::new(ctx);
    KEY_SERVER_EVENTS_LIST.iter().for_each(|callback| {
        callback(&ctx, key_change_sub_event);
    });
}

extern "C" fn server_shutdown_callback(
    ctx: *mut raw::RedisModuleCtx,
    _eid: raw::RedisModuleEvent,
    subevent: u64,
    _data: *mut ::std::os::raw::c_void,
) {
    let ctx = Context::new(ctx);
    SHUTDOWN_SERVER_EVENT_LIST.iter().for_each(|callback| {
        callback(&ctx, subevent);
    });
}

extern "C" fn persistence_event_callback(
    ctx: *mut raw::RedisModuleCtx,
    _eid: raw::RedisModuleEvent,
    subevent: u64,
    _data: *mut ::std::os::raw::c_void,
) {
    let persistence_sub_event = match subevent {
        raw::REDISMODULE_SUBEVENT_PERSISTENCE_RDB_START => PersistenceSubevent::RdbStart,
        raw::REDISMODULE_SUBEVENT_PERSISTENCE_AOF_START => PersistenceSubevent::AofStart,
        raw::REDISMODULE_SUBEVENT_PERSISTENCE_SYNC_RDB_START => PersistenceSubevent::SyncRdbStart,
        raw::REDISMODULE_SUBEVENT_PERSISTENCE_SYNC_AOF_START => PersistenceSubevent::SyncAofStart,
        raw::REDISMODULE_SUBEVENT_PERSISTENCE_ENDED => PersistenceSubevent::Ended,
        raw::REDISMODULE_SUBEVENT_PERSISTENCE_FAILED => PersistenceSubevent::Failed,
        _ => return,
    };
    let ctx = Context::new(ctx);
    PERSISTENCE_SERVER_EVENTS_LIST.iter().for_each(|callback| {
        callback(&ctx, persistence_sub_event);
    });
}

extern "C" fn master_link_change_event_callback(
    ctx: *mut raw::RedisModuleCtx,
    _eid: raw::RedisModuleEvent,
    subevent: u64,
    _data: *mut ::std::os::raw::c_void,
) {
    let master_link_change_sub_event = match subevent {
        raw::REDISMODULE_SUBEVENT_MASTER_LINK_UP => MasterLinkChangeSubevent::Up,
        raw::REDISMODULE_SUBEVENT_MASTER_LINK_DOWN => MasterLinkChangeSubevent::Down,
        _ => return,
    };
    let ctx = Context::new(ctx);
    MASTER_LINK_CHANGE_SERVER_EVENTS_LIST
        .iter()
        .for_each(|callback| {
            callback(&ctx, master_link_change_sub_event);
        });
}

extern "C" fn config_change_event_callback(
    ctx: *mut raw::RedisModuleCtx,
    _eid: raw::RedisModuleEvent,
    _subevent: u64,
    data: *mut ::std::os::raw::c_void,
) {
    let data: &raw::RedisModuleConfigChange =
        unsafe { &*(data as *mut raw::RedisModuleConfigChange) };
    let config_names: Vec<_> = (0..data.num_changes)
        .map(|i| unsafe {
            let name = *data.config_names.offset(i as isize);
            CStr::from_ptr(name)
        })
        .collect();
    let config_names: Vec<_> = config_names
        .iter()
        .map(|v| {
            v.to_str()
                .expect("Got a configuration name which is not a valid utf8")
        })
        .collect();
    let ctx = Context::new(ctx);
    CONFIG_CHANGED_SERVER_EVENTS_LIST
        .iter()
        .for_each(|callback| {
            callback(&ctx, config_names.as_slice());
        });
}

fn register_single_server_event_type<T>(
    ctx: &Context,
    callbacks: &[fn(&Context, T)],
    server_event: u64,
    inner_callback: raw::RedisModuleEventCallback,
) -> Result<(), ValkeyError> {
    if !callbacks.is_empty() {
        // Log which server events we subscribe to so we can debug missing handlers
        ctx.log_notice(&format!(
            "Subscribing to server event id={} (callbacks={})",
            server_event,
            callbacks.len()
        ));
        let res = unsafe {
            raw::RedisModule_SubscribeToServerEvent.unwrap()(
                ctx.ctx,
                raw::RedisModuleEvent {
                    id: server_event,
                    dataver: 1,
                },
                inner_callback,
            )
        };
        if res != raw::REDISMODULE_OK as i32 {
            return Err(ValkeyError::Str("Failed subscribing to server event"));
        }
    }

    Ok(())
}

pub fn register_server_events(ctx: &Context) -> Result<(), ValkeyError> {
    // Debug: print counts of registered callbacks for key server event slices so
    // we can confirm the example handlers were wired into the distributed slices.
    ctx.log_notice(&format!(
        "Server event handlers counts: loading={}, loading_progress={}",
        LOADING_SERVER_EVENTS_LIST.len(), LOADING_PROGRESS_SERVER_EVENTS_LIST.len()
    ));
    register_single_server_event_type(
        ctx,
        &ROLE_CHANGED_SERVER_EVENTS_LIST,
        raw::REDISMODULE_EVENT_REPLICATION_ROLE_CHANGED,
        Some(role_changed_callback),
    )?;
    register_single_server_event_type(
        ctx,
        &LOADING_SERVER_EVENTS_LIST,
        raw::REDISMODULE_EVENT_LOADING,
        Some(loading_event_callback),
    )?;
    register_single_server_event_type(
        ctx,
        &LOADING_PROGRESS_SERVER_EVENTS_LIST,
        raw::REDISMODULE_EVENT_LOADING_PROGRESS,
        Some(loading_progress_event_callback),
    )?;
    register_single_server_event_type(
        ctx,
        &FLUSH_SERVER_EVENTS_LIST,
        raw::REDISMODULE_EVENT_FLUSHDB,
        Some(flush_event_callback),
    )?;
    register_single_server_event_type(
        ctx,
        &MODULE_CHANGED_SERVER_EVENTS_LIST,
        raw::REDISMODULE_EVENT_MODULE_CHANGE,
        Some(module_change_event_callback),
    )?;
    register_single_server_event_type(
        ctx,
        &CLIENT_CHANGED_SERVER_EVENTS_LIST,
        raw::REDISMODULE_EVENT_CLIENT_CHANGE,
        Some(client_change_event_callback),
    )?;
    register_single_server_event_type(
        ctx,
        &CONFIG_CHANGED_SERVER_EVENTS_LIST,
        raw::REDISMODULE_EVENT_CONFIG,
        Some(config_change_event_callback),
    )?;
    register_single_server_event_type(
        ctx,
        &CRON_SERVER_EVENTS_LIST,
        raw::REDISMODULE_EVENT_CRON_LOOP,
        Some(cron_callback),
    )?;
    register_single_server_event_type(
        ctx,
        &KEY_SERVER_EVENTS_LIST,
        raw::REDISMODULE_EVENT_KEY,
        Some(key_event_callback),
    )?;
    register_single_server_event_type(
        ctx,
        &SHUTDOWN_SERVER_EVENT_LIST,
        raw::REDISMODULE_EVENT_SHUTDOWN,
        Some(server_shutdown_callback),
    )?;
    register_single_server_event_type(
        ctx,
        &PERSISTENCE_SERVER_EVENTS_LIST,
        raw::REDISMODULE_EVENT_PERSISTENCE,
        Some(persistence_event_callback),
    )?;
    register_single_server_event_type(
        ctx,
        &MASTER_LINK_CHANGE_SERVER_EVENTS_LIST,
        raw::REDISMODULE_EVENT_MASTER_LINK_CHANGE,
        Some(master_link_change_event_callback),
    )?;
    Ok(())
}
