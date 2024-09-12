use lazy_static::lazy_static;
use std::mem::drop;
use std::thread;
use std::time::Duration;
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{
    valkey_module, Context, NextArg, ThreadSafeContext, ValkeyGILGuard, ValkeyResult, ValkeyString,
    ValkeyValue,
};

fn threads(_: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    thread::spawn(move || {
        let thread_ctx = ThreadSafeContext::new();

        loop {
            let ctx = thread_ctx.lock();
            ctx.call("INCR", &["threads"]).unwrap();
            // release the lock as soon as we're done accessing valkey memory
            drop(ctx);
            thread::sleep(Duration::from_millis(1000));
        }
    });

    Ok(().into())
}

#[derive(Default)]
struct StaticData {
    data: String,
}

lazy_static! {
    static ref STATIC_DATA: ValkeyGILGuard<StaticData> = ValkeyGILGuard::new(StaticData::default());
}

fn set_static_data(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    let mut args = args.into_iter().skip(1);
    let val = args.next_str()?;
    let mut static_data = STATIC_DATA.lock(ctx);
    static_data.data = val.to_string();
    Ok(ValkeyValue::SimpleStringStatic("OK"))
}

fn get_static_data(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    let static_data = STATIC_DATA.lock(ctx);
    Ok(ValkeyValue::BulkString(static_data.data.clone()))
}

fn get_static_data_on_thread(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    let blocked_client = ctx.block_client();
    let _ = thread::spawn(move || {
        let thread_ctx = ThreadSafeContext::with_blocked_client(blocked_client);
        let ctx = thread_ctx.lock();
        let static_data = STATIC_DATA.lock(&ctx);
        thread_ctx.reply(Ok(static_data.data.clone().into()));
    });

    Ok(ValkeyValue::NoReply)
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "threads",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["threads", threads, "", 0, 0, 0],
        ["set_static_data", set_static_data, "", 0, 0, 0],
        ["get_static_data", get_static_data, "", 0, 0, 0],
        ["get_static_data_on_thread", get_static_data_on_thread, "", 0, 0, 0],
    ],
}
