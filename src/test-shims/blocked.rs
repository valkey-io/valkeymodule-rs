use crate::raw;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::os::raw::{c_int, c_longlong, c_void};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

/// Installs the raw blocked-client callbacks used by the test shim.
pub(super) fn install() {
    // SAFETY: `setup_test_shims` calls this once after verifying the real API is uninitialized.
    unsafe {
        raw::RedisModule_BlockClient = Some(block_client);
        raw::RedisModule_UnblockClient = Some(unblock_client);
        raw::RedisModule_AbortBlock = Some(abort_block);
    }
}

// `Context::block_client` is called synchronously on the thread that owns the test context, so
// pending fixture registration follows the other context shims and remains thread-local. A live
// blocked client may move to a background thread, so its state uses a synchronized registry.
thread_local! {
    static PENDING_BLOCKED_CLIENTS: RefCell<HashMap<usize, VecDeque<SharedBlockedClientState>>> = RefCell::default();
}

static LIVE_BLOCKED_CLIENTS: OnceLock<Mutex<HashMap<usize, BlockedClientHandle>>> = OnceLock::new();
static NEXT_BLOCKED_CLIENT_TOKEN: AtomicUsize = AtomicUsize::new(1);

type SharedBlockedClientState = Arc<Mutex<BlockedClientState>>;

/// Records the observable lifecycle of one blocked-client fixture.
#[derive(Default)]
struct BlockedClientState {
    unblocked: bool,
    aborted: bool,
    thread_safe_context_count: usize,
}

/// Identifies the terminal operation applied to a blocked-client fixture.
enum TerminalOperation {
    Unblock,
    Abort,
}

/// Implements fixture-state transitions shared by the raw client callbacks.
impl BlockedClientState {
    /// Records a terminal operation after verifying that the fixture is still live.
    fn complete(&mut self, operation: TerminalOperation) {
        assert!(
            !self.unblocked && !self.aborted,
            "blocked-client fixture was already completed"
        );
        match operation {
            TerminalOperation::Unblock => self.unblocked = true,
            TerminalOperation::Abort => self.aborted = true,
        }
    }
}

/// Retains the fixture state while its opaque raw handle is live.
struct BlockedClientHandle {
    state: SharedBlockedClientState,
}

/// Observes the terminal state of one test-only blocked client.
#[derive(Clone)]
pub struct TestBlockedClient {
    state: SharedBlockedClientState,
}

/// Exposes read-only lifecycle observations to tests.
impl TestBlockedClient {
    /// Returns whether dropping the blocked client unblocked the fixture.
    #[must_use]
    pub fn was_unblocked(&self) -> bool {
        with_state(&self.state, |state| state.unblocked)
    }

    /// Returns whether `BlockedClient::abort` completed the fixture.
    #[must_use]
    pub fn was_aborted(&self) -> bool {
        with_state(&self.state, |state| state.aborted)
    }

    /// Returns how many thread-safe contexts were created from this client.
    #[must_use]
    pub fn thread_safe_context_count(&self) -> usize {
        with_state(&self.state, |state| state.thread_safe_context_count)
    }
}

/// Queues a blocked-client fixture for the supplied test context.
pub(super) fn expect_block_client(ctx: *mut raw::RedisModuleCtx) -> TestBlockedClient {
    let state = Arc::new(Mutex::new(BlockedClientState::default()));
    PENDING_BLOCKED_CLIENTS.with(|pending| {
        pending
            .borrow_mut()
            .entry(ctx as usize)
            .or_default()
            .push_back(Arc::clone(&state));
    });
    TestBlockedClient { state }
}

/// Drops pending blocked-client fixtures when their test context is released.
pub(super) fn forget_context(ctx: *mut raw::RedisModuleCtx) {
    PENDING_BLOCKED_CLIENTS.with(|pending| {
        pending.borrow_mut().remove(&(ctx as usize));
    });
}

/// Creates a test thread-safe context associated with a live blocked client.
pub(super) fn thread_safe_context(
    blocked_client: *mut raw::RedisModuleBlockedClient,
) -> *mut raw::RedisModuleCtx {
    let token = blocked_client as usize;
    let state = lock_live()
        .get(&token)
        .map(|handle| Arc::clone(&handle.state))
        .unwrap_or_else(|| panic!("blocked-client fixture is invalid or already completed"));
    with_state(&state, |state| state.thread_safe_context_count += 1);
    super::thread_safe::register_new()
}

/// Allocates an opaque raw handle from the next fixture configured on `ctx`.
extern "C" fn block_client(
    ctx: *mut raw::RedisModuleCtx,
    _reply_callback: raw::RedisModuleCmdFunc,
    _timeout_callback: raw::RedisModuleCmdFunc,
    _free_privdata: Option<unsafe extern "C" fn(*mut raw::RedisModuleCtx, *mut c_void)>,
    _timeout_ms: c_longlong,
) -> *mut raw::RedisModuleBlockedClient {
    let state = PENDING_BLOCKED_CLIENTS.with(|pending| {
        pending
            .borrow_mut()
            .get_mut(&(ctx as usize))
            .and_then(VecDeque::pop_front)
    });
    let state = state
        .unwrap_or_else(|| panic!("no blocked-client fixture configured for this test context"));
    let token = NEXT_BLOCKED_CLIENT_TOKEN
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |token| {
            token.checked_add(2)
        })
        .expect("blocked-client test token space exhausted");
    let blocked_client = token as *mut raw::RedisModuleBlockedClient;
    let replaced = lock_live().insert(token, BlockedClientHandle { state });
    assert!(
        replaced.is_none(),
        "blocked-client test token should be unique"
    );
    blocked_client
}

/// Completes a fixture through the normal unblock path.
extern "C" fn unblock_client(
    blocked_client: *mut raw::RedisModuleBlockedClient,
    _private_data: *mut c_void,
) -> c_int {
    complete(blocked_client, TerminalOperation::Unblock);
    raw::REDISMODULE_OK as c_int
}

/// Completes a fixture through the explicit abort path.
extern "C" fn abort_block(blocked_client: *mut raw::RedisModuleBlockedClient) -> c_int {
    complete(blocked_client, TerminalOperation::Abort);
    raw::REDISMODULE_OK as c_int
}

/// Applies one terminal lifecycle transition and retires its raw handle.
fn complete(blocked_client: *mut raw::RedisModuleBlockedClient, operation: TerminalOperation) {
    let token = blocked_client as usize;
    let handle = lock_live()
        .remove(&token)
        .unwrap_or_else(|| panic!("blocked-client fixture was already completed or is invalid"));
    with_state(&handle.state, |state| state.complete(operation));
}

/// Returns the process-wide registry of live opaque blocked-client handles.
fn live() -> &'static Mutex<HashMap<usize, BlockedClientHandle>> {
    LIVE_BLOCKED_CLIENTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Locks the live-handle registry, recovering its state after a test panic.
fn lock_live() -> MutexGuard<'static, HashMap<usize, BlockedClientHandle>> {
    live()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Locks one fixture's state, recovering it after a test panic.
fn lock_state(state: &SharedBlockedClientState) -> MutexGuard<'_, BlockedClientState> {
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Runs an operation while holding one fixture's state lock.
fn with_state<T>(
    state: &SharedBlockedClientState,
    operation: impl FnOnce(&mut BlockedClientState) -> T,
) -> T {
    operation(&mut lock_state(state))
}

#[cfg(test)]
mod tests {
    use crate::{Context, ThreadSafeContext};

    #[test]
    fn block_client_drop_unblocks_fixture() {
        let mut context = Context::test();
        let fixture = context.expect_block_client();

        drop(context.block_client());

        assert!(fixture.was_unblocked());
        assert!(!fixture.was_aborted());
    }

    #[test]
    fn block_client_abort_marks_fixture_aborted() {
        let mut context = Context::test();
        let fixture = context.expect_block_client();

        context
            .block_client()
            .abort()
            .expect("test abort should succeed");

        assert!(fixture.was_aborted());
        assert!(!fixture.was_unblocked());
    }

    #[test]
    fn blocked_client_creates_thread_safe_context() {
        let mut context = Context::test();
        let fixture = context.expect_block_client();
        let blocked_client = context.block_client();

        drop(ThreadSafeContext::with_blocked_client(blocked_client));

        assert_eq!(fixture.thread_safe_context_count(), 1);
        assert!(fixture.was_unblocked());
    }
}
