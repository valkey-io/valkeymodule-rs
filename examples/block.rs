use std::thread;
use std::time::Duration;
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{
    valkey_module, Context, ThreadSafeContext, ValkeyResult, ValkeyString, ValkeyValue,
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

//////////////////////////////////////////////////////

valkey_module! {
    name: "block",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["block", block, "", 0, 0, 0],
    ],
}
