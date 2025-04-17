use std::sync::atomic::{AtomicI64, Ordering};

use valkey_module::alloc::ValkeyAlloc;
use valkey_module::server_events::{ClientChangeSubevent, KeyChangeSubevent};
use valkey_module::{
    server_events::FlushSubevent, valkey_module, Context, ValkeyResult, ValkeyString, ValkeyValue,
};
use valkey_module_macros::{
    client_changed_event_handler, config_changed_event_handler, cron_event_handler,
    flush_event_handler, key_event_handler, shutdown_event_handler,
};

static NUM_FLUSHES: AtomicI64 = AtomicI64::new(0);
static NUM_CONNECTS: AtomicI64 = AtomicI64::new(0);
static NUM_CRONS: AtomicI64 = AtomicI64::new(0);
static NUM_MAX_MEMORY_CONFIGURATION_CHANGES: AtomicI64 = AtomicI64::new(0);
static NUM_KEY_EVENTS: AtomicI64 = AtomicI64::new(0);

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

//////////////////////////////////////////////////////

valkey_module! {
    name: "srv_events",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["num_flushed", num_flushed, "readonly", 0, 0, 0],
        ["num_max_memory_changes", num_maxmemory_changes, "readonly", 0, 0, 0],
        ["num_crons", num_crons, "readonly", 0, 0, 0],
        ["num_connects", num_connects, "readonly", 0, 0, 0],
        ["num_key_events", num_key_events, "readonly", 0, 0, 0],
    ]
}
