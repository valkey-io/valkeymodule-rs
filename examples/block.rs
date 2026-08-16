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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use valkey_module::test_shims::TestBlockedClient;

    #[test]
    fn block_command_releases_its_blocked_client() {
        let mut context = Context::test();
        let fixture = context.expect_block_client();

        assert!(matches!(
            block(&context, Vec::new()),
            Ok(ValkeyValue::NoReply)
        ));

        wait_for_unblock(&fixture);
        assert_eq!(fixture.thread_safe_context_count(), 1);
        assert!(!fixture.was_aborted());
    }

    fn wait_for_unblock(fixture: &TestBlockedClient) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !fixture.was_unblocked() {
            assert!(Instant::now() < deadline, "blocked client was not released");
            thread::sleep(Duration::from_millis(10));
        }
    }
}
