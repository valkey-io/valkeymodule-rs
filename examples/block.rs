use std::thread;
use std::time::Duration;
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{
    valkey_module, Context, ThreadSafeContext, ValkeyResult, ValkeyString, ValkeyStringBuf,
    ValkeyValue,
};

fn block(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    let blocked_client = ctx.block_client();

    thread::spawn(move || {
        let thread_ctx = ThreadSafeContext::with_blocked_client(blocked_client);
        thread::sleep(Duration::from_millis(1000));
        thread_ctx.reply(Ok("42".into()));
    });

    // We will reply later, from the thread
    Ok(ValkeyValue::NoReply)
}

fn block_measured(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    let mut blocked_client = ctx.block_client();
    // Count the thread's work toward the command's stats.
    blocked_client.measure_time_start();

    thread::spawn(move || {
        let thread_ctx = ThreadSafeContext::with_blocked_client(blocked_client);
        thread::sleep(Duration::from_millis(200));
        thread_ctx.reply(Ok("42".into()));
    });

    Ok(ValkeyValue::NoReply)
}

fn reply_on_unblock(_ctx: &Context, data: Option<ValkeyStringBuf>) -> ValkeyResult {
    match data {
        // Sent by reference, no copy.
        Some(buf) => Ok(ValkeyValue::BulkValkeyString(buf.into_valkey_string())),
        None => Ok(ValkeyValue::Null),
    }
}

fn block_reply_callback(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    let blocked_client = ctx.block_client_with_callbacks(reply_on_unblock, None);

    thread::spawn(move || {
        let thread_ctx = ThreadSafeContext::with_blocked_client(blocked_client);
        thread::sleep(Duration::from_millis(200));
        let mut buf = ValkeyStringBuf::with_len(2);
        buf.as_mut_slice().copy_from_slice(b"88");
        thread_ctx.set_blocked_private_data(buf).unwrap();
    });

    Ok(ValkeyValue::NoReply)
}

/// `block.lock_set key value`: SET from a thread under the lock, in the client's db.
fn block_lock_set(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() != 3 {
        return Err(valkey_module::ValkeyError::WrongArity);
    }
    let (key, value) = (args[1].as_slice().to_vec(), args[2].as_slice().to_vec());
    let blocked_client = ctx.block_client();

    thread::spawn(move || {
        let thread_ctx = ThreadSafeContext::with_blocked_client(blocked_client);
        thread_ctx.with_lock(|ctx| ctx.call("SET", &[&key[..], &value[..]]).unwrap());
        thread_ctx.reply(Ok("OK".into()));
    });

    Ok(ValkeyValue::NoReply)
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "block",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["block", block, "", 0, 0, 0],
        ["block.measured", block_measured, "", 0, 0, 0],
        ["block.reply_callback", block_reply_callback, "", 0, 0, 0],
        ["block.lock_set", block_lock_set, "write", 1, 1, 1],
    ],
}
