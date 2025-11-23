use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use valkey_module::alloc::ValkeyAlloc;
use valkey_module::server_events::{
    ClientChangeSubevent,
    KeyChangeSubevent,
    MasterLinkChangeSubevent,
    PersistenceSubevent,
    LoadingSubevent,
    LoadingProgress,
};
use valkey_module::{
    server_events::FlushSubevent, valkey_module, Context, ValkeyResult, ValkeyString, ValkeyValue,
};
use valkey_module_macros::{
    client_changed_event_handler,
    config_changed_event_handler,
    cron_event_handler,
    flush_event_handler,
    key_event_handler,
    master_link_change_event_handler,
    persistence_event_handler,
    shutdown_event_handler,
    loading_event_handler,
    loading_progress_event_handler,
};

static NUM_FLUSHES: AtomicI64 = AtomicI64::new(0);
static NUM_CONNECTS: AtomicI64 = AtomicI64::new(0);
static NUM_CRONS: AtomicI64 = AtomicI64::new(0);
static NUM_MAX_MEMORY_CONFIGURATION_CHANGES: AtomicI64 = AtomicI64::new(0);
static NUM_KEY_EVENTS: AtomicI64 = AtomicI64::new(0);
static NUM_PERSISTENCE_EVENTS: AtomicI64 = AtomicI64::new(0);
static NUM_MASTER_LINK_CHANGE_EVENTS: AtomicI64 = AtomicI64::new(0);
static IS_MASTER_LINK_UP: AtomicBool = AtomicBool::new(false);
static NUM_LOADING_PROGRESS_RDB: AtomicI64 = AtomicI64::new(0);
static NUM_LOADING_PROGRESS_AOF: AtomicI64 = AtomicI64::new(0);

#[flush_event_handler]
fn flushed_event_handler(_ctx: &Context, flush_event: FlushSubevent) {
    if let FlushSubevent::Started = flush_event {
        NUM_FLUSHES.fetch_add(1, Ordering::SeqCst);
    }
}

#[config_changed_event_handler]
fn config_changed_event_handler(_ctx: &Context, changed_configs: &[&str]) {
    changed_configs
        .iter()
        .find(|v| **v == "maxmemory")
        .map(|_| NUM_MAX_MEMORY_CONFIGURATION_CHANGES.fetch_add(1, Ordering::SeqCst));
}

#[cron_event_handler]
fn cron_event_handler(_ctx: &Context, _hz: u64) {
    NUM_CRONS.fetch_add(1, Ordering::SeqCst);
}

#[client_changed_event_handler]
fn client_changed_event_handler(ctx: &Context, client_event: ClientChangeSubevent) {
    match client_event {
        ClientChangeSubevent::Connected => {
            ctx.log_notice("Connected");
            NUM_CONNECTS.fetch_add(1, Ordering::SeqCst);
        }
        ClientChangeSubevent::Disconnected => {
            ctx.log_notice("Disconnected");
            NUM_CONNECTS.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

#[key_event_handler]
fn key_event_handler(ctx: &Context, key_event: KeyChangeSubevent) {
    match key_event {
        KeyChangeSubevent::Deleted => {
            ctx.log_notice("Key deleted");
        }
        KeyChangeSubevent::Evicted => {
            ctx.log_notice("Key evicted");
        }
        KeyChangeSubevent::Overwritten => {
            ctx.log_notice("Key overwritten");
        }
        KeyChangeSubevent::Expired => {
            ctx.log_notice("Key expired");
        }
    }
    NUM_KEY_EVENTS.fetch_add(1, Ordering::SeqCst);
}

#[shutdown_event_handler]
fn shutdown_event_handler(ctx: &Context, _event: u64) {
    ctx.log_notice("Sever shutdown callback event ...");
    // Check if test file shutdown_log.txt exists and wrie the above log to it
    let shutdown_log_path = "shutdown_log.txt";

    // Attempt to write the log message to the file
    if let Err(e) = std::fs::write(shutdown_log_path, "Server shutdown callback event ...\n") {
        ctx.log_warning(&format!("Failed to write to shutdown log file: {}", e));
    }
}

#[persistence_event_handler]
fn persistence_event_handler(ctx: &Context, persistence_event: PersistenceSubevent) {
    match persistence_event {
        PersistenceSubevent::RdbStart => {
            ctx.log_notice("RDB persistence started");
        }
        PersistenceSubevent::AofStart => {
            ctx.log_notice("AOF persistence started");
        }
        PersistenceSubevent::SyncRdbStart => {
            ctx.log_notice("Sync RDB persistence started");
        }
        PersistenceSubevent::SyncAofStart => {
            ctx.log_notice("Sync AOF persistence started");
        }
        PersistenceSubevent::Ended => {
            ctx.log_notice("Persistence operation ended");
        }
        PersistenceSubevent::Failed => {
            ctx.log_warning("Persistence operation failed");
        }
    }
    NUM_PERSISTENCE_EVENTS.fetch_add(1, Ordering::SeqCst);
}

#[loading_event_handler]
fn loading_event_handler(ctx: &Context, ev: LoadingSubevent) {
    match ev {
        LoadingSubevent::RdbStarted => ctx.log_notice("Loading RDB started"),
        LoadingSubevent::AofStarted => ctx.log_notice("Loading AOF started"),
        LoadingSubevent::ReplStarted => ctx.log_notice("Replication loading started"),
        LoadingSubevent::Ended => ctx.log_notice("Loading ended"),
        LoadingSubevent::Failed => ctx.log_warning("Loading failed"),
    }
}

#[loading_progress_event_handler]
fn loading_progress_event_handler(ctx: &Context, info: LoadingProgress) {
    let msg = format!(
        "Loading progress {:?}: hz={}, progress={}",
        info.subevent, info.hz, info.progress
    );
    ctx.log_notice(&msg);
    match info.subevent {
        valkey_module::server_events::LoadingProgressSubevent::Rdb => {
            NUM_LOADING_PROGRESS_RDB.fetch_add(1, Ordering::SeqCst);
        }
        valkey_module::server_events::LoadingProgressSubevent::Aof => {
            NUM_LOADING_PROGRESS_AOF.fetch_add(1, Ordering::SeqCst);
        }
    }
}

#[master_link_change_event_handler]
fn master_link_change_event_handler(
    ctx: &Context,
    master_link_change_subevent: MasterLinkChangeSubevent,
) {
    match master_link_change_subevent {
        MasterLinkChangeSubevent::Up => {
            ctx.log_warning("Master link status up");
            NUM_MASTER_LINK_CHANGE_EVENTS.fetch_add(1, Ordering::SeqCst);
            IS_MASTER_LINK_UP.store(true, Ordering::SeqCst);
        }
        MasterLinkChangeSubevent::Down => {
            ctx.log_warning("Master link status down");
            NUM_MASTER_LINK_CHANGE_EVENTS.fetch_add(1, Ordering::SeqCst);
            IS_MASTER_LINK_UP.store(false, Ordering::SeqCst);
        }
    }
}

fn num_flushed(_ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ValkeyValue::Integer(NUM_FLUSHES.load(Ordering::SeqCst)))
}

fn num_crons(_ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ValkeyValue::Integer(NUM_CRONS.load(Ordering::SeqCst)))
}

fn num_maxmemory_changes(_ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ValkeyValue::Integer(
        NUM_MAX_MEMORY_CONFIGURATION_CHANGES.load(Ordering::SeqCst),
    ))
}

fn num_connects(_ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ValkeyValue::Integer(NUM_CONNECTS.load(Ordering::SeqCst)))
}

fn num_key_events(_ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ValkeyValue::Integer(NUM_KEY_EVENTS.load(Ordering::SeqCst)))
}

fn num_master_link_change_events(_ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ValkeyValue::Integer(
        NUM_MASTER_LINK_CHANGE_EVENTS.load(Ordering::SeqCst),
    ))
}

fn is_master_link_up(_ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ValkeyValue::Bool(IS_MASTER_LINK_UP.load(Ordering::SeqCst)))
}

fn num_persistence_events(_ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ValkeyValue::Integer(
        NUM_PERSISTENCE_EVENTS.load(Ordering::SeqCst),
    ))
}

fn num_loading_progress_rdb(_ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ValkeyValue::Integer(NUM_LOADING_PROGRESS_RDB.load(Ordering::SeqCst)))
}

fn num_loading_progress_aof(_ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ValkeyValue::Integer(NUM_LOADING_PROGRESS_AOF.load(Ordering::SeqCst)))
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "srv_events",
    version: 2,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["num_flushed", num_flushed, "readonly", 0, 0, 0],
        ["num_max_memory_changes", num_maxmemory_changes, "readonly", 0, 0, 0],
        ["num_crons", num_crons, "readonly", 0, 0, 0],
        ["num_connects", num_connects, "readonly", 0, 0, 0],
        ["num_key_events", num_key_events, "readonly", 0, 0, 0],
        ["num_persistence_events", num_persistence_events, "readonly", 0, 0, 0],
        ["num_master_link_change_events", num_master_link_change_events, "readonly", 0, 0, 0],
        ["is_master_link_up", is_master_link_up, "readonly", 0, 0, 0],
        ["num_loading_progress_rdb", num_loading_progress_rdb, "readonly", 0, 0, 0],
        ["num_loading_progress_aof", num_loading_progress_aof, "readonly", 0, 0, 0],
    ]
}
