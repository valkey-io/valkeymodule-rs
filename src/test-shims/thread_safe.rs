use super::call::TestCallExpectation;
use super::context::TestContext;
use crate::context::thread_safe::{DetachedFromClient, ThreadSafeContext};
use crate::{raw, ValkeyValue};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

pub(super) fn install() {
    // SAFETY: `setup_test_shims` calls this once after verifying the real API is uninitialized.
    unsafe {
        raw::RedisModule_GetThreadSafeContext = Some(get_thread_safe_context);
        raw::RedisModule_ThreadSafeContextLock = Some(thread_safe_context_lock);
        raw::RedisModule_ThreadSafeContextUnlock = Some(thread_safe_context_unlock);
        raw::RedisModule_FreeThreadSafeContext = Some(free_thread_safe_context);
    }
}

// Maps live base context tokens to synchronized fixture state so expectations can be configured
// on one thread and installed into an ordinary test context on the thread that locks it.
static THREAD_SAFE_CONTEXTS: OnceLock<Mutex<HashMap<usize, SharedContextData>>> = OnceLock::new();
static NEXT_TEST_CONTEXT_TOKEN: AtomicUsize = AtomicUsize::new(1);

thread_local! {
    /// Associates a lock operation with the next guard context created on the same thread.
    static PENDING_LOCK_STATE: RefCell<PendingLockState> = RefCell::default();
    /// Keeps each ordinary test context alive until its corresponding guard is freed.
    static LIVE_GUARD_CONTEXTS: RefCell<HashMap<usize, TestContext>> = RefCell::default();
}

impl ThreadSafeContext<DetachedFromClient> {
    /// Creates a configurable thread-safe context for tests without a running Valkey server.
    ///
    /// The test shim supports only thread-safe contexts detached from blocked clients. It does
    /// not simulate a thread-safe context associated with a [`crate::BlockedClient`].
    #[must_use]
    pub fn test() -> TestThreadSafeContext {
        TestThreadSafeContext::new()
    }
}

/// Stores expectations owned by one test thread-safe context.
#[derive(Clone, Default)]
struct ThreadSafeContextData {
    client_id: Option<u64>,
    call_expectations: Vec<TestCallExpectation>,
}

/// Provides synchronized ownership of context expectations across test threads.
type SharedContextData = Arc<Mutex<ThreadSafeContextData>>;

/// Distinguishes a standalone context request from valid and invalid lock requests.
#[derive(Default)]
enum PendingLockState {
    #[default]
    Unlocked,
    Valid(SharedContextData),
    Invalid,
}

/// Owns a configurable test-only detached [`ThreadSafeContext`] backed by synchronized state.
pub struct TestThreadSafeContext {
    context: ThreadSafeContext<DetachedFromClient>,
    data: SharedContextData,
}

// Constructs thread-safe test contexts and configures their shared callback expectations.
impl TestThreadSafeContext {
    fn new() -> Self {
        super::setup_test_shims();

        let context = ThreadSafeContext::new();
        let data = state(context.ctx).expect("new test thread-safe context should be registered");
        Self { context, data }
    }

    /// Configures the value returned by [`crate::Context::get_client_id`].
    pub fn expect_get_client_id(&mut self, client_id: u64) -> &mut Self {
        lock_state(&self.data).client_id = Some(client_id);
        self
    }

    /// Configures a reply for an exact command and argument list.
    ///
    /// Expectations form a reusable fixture template. Each lock receives a fresh copy, so the
    /// configured reply remains available across repeated and concurrent locks.
    pub fn expect_call<T: AsRef<[u8]>>(
        &mut self,
        command: impl AsRef<[u8]>,
        args: &[T],
        reply: ValkeyValue,
    ) -> &mut Self {
        let expectation = TestCallExpectation::new(command, args, reply)
            .expect("unsupported reply type configured for test-shim call");
        lock_state(&self.data).call_expectations.push(expectation);
        self
    }

    /// Configures the value returned by [`crate::Context::config_get`].
    pub fn expect_config_get(
        &mut self,
        config: impl Into<String>,
        value: impl Into<String>,
    ) -> &mut Self {
        let config = config.into();
        self.expect_call(
            "CONFIG",
            &["GET", config.as_str()],
            ValkeyValue::Array(vec![
                ValkeyValue::SimpleString(config.clone()),
                ValkeyValue::SimpleString(value.into()),
            ]),
        )
    }
}

impl Deref for TestThreadSafeContext {
    type Target = ThreadSafeContext<DetachedFromClient>;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

/// Implements detached test contexts only; the blocked-client association is not shimmed.
pub(super) extern "C" fn get_thread_safe_context(
    _blocked_client: *mut raw::RedisModuleBlockedClient,
) -> *mut raw::RedisModuleCtx {
    let pending = PENDING_LOCK_STATE.with(|pending| std::mem::take(&mut *pending.borrow_mut()));
    match pending {
        PendingLockState::Unlocked => register_new(),
        PendingLockState::Valid(data) => create_guard_context(data),
        PendingLockState::Invalid => std::ptr::null_mut(),
    }
}

pub(super) extern "C" fn thread_safe_context_lock(ctx: *mut raw::RedisModuleCtx) {
    PENDING_LOCK_STATE.with(|pending| {
        *pending.borrow_mut() = state(ctx)
            .map(PendingLockState::Valid)
            .unwrap_or(PendingLockState::Invalid);
    });
}

pub(super) extern "C" fn thread_safe_context_unlock(_ctx: *mut raw::RedisModuleCtx) {
    PENDING_LOCK_STATE.with(|pending| {
        *pending.borrow_mut() = PendingLockState::Unlocked;
    });
}

pub(super) extern "C" fn free_thread_safe_context(ctx: *mut raw::RedisModuleCtx) {
    let guard = LIVE_GUARD_CONTEXTS.with(|contexts| contexts.borrow_mut().remove(&(ctx as usize)));
    if guard.is_none() {
        unregister(ctx);
    }
}

/// Allocates a new opaque thread-safe context token with default synchronized state.
fn register_new() -> *mut raw::RedisModuleCtx {
    register_shared(Arc::new(Mutex::new(ThreadSafeContextData::default())))
}

/// Issues a unique opaque thread-safe context token that resolves to existing state.
fn register_shared(data: SharedContextData) -> *mut raw::RedisModuleCtx {
    // The token is never dereferenced. Odd monotonic values cannot collide with aligned ordinary
    // test-context pointers and prevent stale tokens from resolving to replacement contexts.
    let token = NEXT_TEST_CONTEXT_TOKEN
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |token| {
            token.checked_add(2)
        })
        .expect("thread-safe test context token space exhausted");
    let ctx = token as *mut raw::RedisModuleCtx;
    lock_registry().insert(ctx as usize, data);
    ctx
}

/// Resolves a live opaque thread-safe context token to its synchronized state.
fn state(ctx: *mut raw::RedisModuleCtx) -> Option<SharedContextData> {
    if ctx.is_null() {
        return None;
    }
    lock_registry().get(&(ctx as usize)).cloned()
}

/// Unregisters a thread-safe context and retires its opaque token.
fn unregister(ctx: *mut raw::RedisModuleCtx) {
    if !ctx.is_null() {
        lock_registry().remove(&(ctx as usize));
    }
}

fn registry() -> &'static Mutex<HashMap<usize, SharedContextData>> {
    THREAD_SAFE_CONTEXTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_registry() -> MutexGuard<'static, HashMap<usize, SharedContextData>> {
    registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn lock_state(data: &SharedContextData) -> MutexGuard<'_, ThreadSafeContextData> {
    data.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Creates an ordinary test context on the locking thread and replays all configured expectations.
fn create_guard_context(data: SharedContextData) -> *mut raw::RedisModuleCtx {
    let data = lock_state(&data).clone();
    let mut context = TestContext::new_thread_safe_guard();

    if let Some(client_id) = data.client_id {
        context.expect_get_client_id(client_id);
    }
    for expectation in data.call_expectations {
        context.expect_call_reply(expectation);
    }

    let ctx = context.ctx;
    let replaced =
        LIVE_GUARD_CONTEXTS.with(|contexts| contexts.borrow_mut().insert(ctx as usize, context));
    assert!(
        replaced.is_none(),
        "new guard context should have a unique pointer"
    );
    ctx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Context, RedisModuleClientInfo, ValkeyError, ValkeyValue};
    use std::ptr::null_mut;
    use std::sync::Barrier;

    #[test]
    fn creates_registered_test_context_for_guard() {
        let base = get_thread_safe_context(null_mut());
        thread_safe_context_lock(base);
        let guard = get_thread_safe_context(null_mut());

        assert!(state(base).is_some());
        assert!(state(guard).is_none());
        assert!(
            LIVE_GUARD_CONTEXTS.with(|contexts| contexts.borrow().contains_key(&(guard as usize)))
        );

        thread_safe_context_unlock(guard);
        free_thread_safe_context(guard);
        assert!(
            !LIVE_GUARD_CONTEXTS.with(|contexts| contexts.borrow().contains_key(&(guard as usize)))
        );

        free_thread_safe_context(base);
        assert!(state(base).is_none());
    }

    #[test]
    fn retired_context_token_is_not_reused() {
        let retired = get_thread_safe_context(null_mut());
        free_thread_safe_context(retired);

        let replacement = get_thread_safe_context(null_mut());

        assert_ne!(retired, replacement);
        assert!(state(retired).is_none());
        free_thread_safe_context(replacement);
    }

    #[test]
    fn invalid_lock_for_null_handle_is_rejected() {
        assert_invalid_lock_is_rejected(null_mut());
    }

    #[test]
    fn invalid_lock_for_foreign_handle_is_rejected() {
        let foreign = 2usize as *mut raw::RedisModuleCtx;

        assert_invalid_lock_is_rejected(foreign);
    }

    #[test]
    fn invalid_lock_for_retired_handle_is_rejected() {
        let retired = get_thread_safe_context(null_mut());
        free_thread_safe_context(retired);

        assert_invalid_lock_is_rejected(retired);
    }

    #[test]
    fn invalid_lock_for_double_freed_handle_is_rejected() {
        let retired = get_thread_safe_context(null_mut());
        free_thread_safe_context(retired);
        free_thread_safe_context(retired);

        assert_invalid_lock_is_rejected(retired);
    }

    #[test]
    fn test_thread_safe_context_configures_locked_context() {
        let mut context = ThreadSafeContext::test();
        context
            .expect_get_client_id(42)
            .expect_call("INCR", &["counter"], ValkeyValue::Integer(1))
            .expect_config_get("hz", "10");

        let guard = context.lock();

        assert_eq!(guard.get_client_id(), 42);
        assert!(matches!(
            guard.call("INCR", &["counter"]),
            Ok(ValkeyValue::Integer(1))
        ));
        assert_eq!(
            guard
                .config_get("hz".to_owned())
                .expect("configured config value should be returned")
                .as_slice(),
            b"10"
        );
    }

    #[test]
    fn independent_thread_safe_contexts_do_not_share_fixture_state() {
        let mut first = ThreadSafeContext::test();
        first
            .expect_get_client_id(11)
            .expect_call("FIRST", &["one"], ValkeyValue::Integer(1));
        let mut second = ThreadSafeContext::test();
        second
            .expect_get_client_id(22)
            .expect_call("SECOND", &["two"], ValkeyValue::Integer(2));

        let first_guard = first.lock();
        assert_eq!(first_guard.get_client_id(), 11);
        assert_eq!(
            first_guard
                .call("FIRST", &["one"])
                .expect("first fixture should return its configured reply"),
            ValkeyValue::Integer(1)
        );

        let second_guard = second.lock();
        assert_eq!(second_guard.get_client_id(), 22);
        assert_eq!(
            second_guard
                .call("SECOND", &["two"])
                .expect("second fixture should return its configured reply"),
            ValkeyValue::Integer(2)
        );
        assert!(matches!(
            first_guard.call("SECOND", &["two"]),
            Err(ValkeyError::String(message)) if message == "unexpected call: SECOND two"
        ));
        assert!(matches!(
            second_guard.call("FIRST", &["one"]),
            Err(ValkeyError::String(message)) if message == "unexpected call: FIRST one"
        ));

        drop(second_guard);
        assert_eq!(first_guard.get_client_id(), 11);
    }

    #[test]
    fn dropping_public_thread_safe_context_retires_registry_entry() {
        let context = ThreadSafeContext::test();
        let ctx = context.context.ctx;
        assert!(state(ctx).is_some());

        drop(context);

        assert!(state(ctx).is_none());
    }

    #[test]
    fn guards_are_cleaned_up_independently_in_reverse_order() {
        let mut first = ThreadSafeContext::test();
        first.expect_get_client_id(11);
        let mut second = ThreadSafeContext::test();
        second.expect_get_client_id(22);
        let first_guard = first.lock();
        let second_guard = second.lock();
        let first_ctx = first_guard.ctx;
        let second_ctx = second_guard.ctx;

        assert!(guard_is_live(first_ctx));
        assert!(guard_is_live(second_ctx));

        drop(second_guard);
        assert!(guard_is_live(first_ctx));
        assert!(!guard_is_live(second_ctx));
        assert_eq!(first_guard.get_client_id(), 11);

        drop(first_guard);
        assert!(!guard_is_live(first_ctx));
    }

    #[test]
    fn locking_thread_safe_context_preserves_ordinary_context_expectations() {
        let mut ordinary_context = Context::test();
        ordinary_context
            .expect_get_server_version(8, 1, 2)
            .expect_get_client_name_by_id(42, "alice")
            .expect_get_client_info_by_id(RedisModuleClientInfo {
                id: 42,
                port: 6379,
                ..RedisModuleClientInfo::default()
            });
        let thread_safe_context = ThreadSafeContext::test();

        drop(thread_safe_context.lock());

        assert_eq!(
            ordinary_context
                .get_server_version()
                .expect("configured server version should be preserved"),
            raw::Version {
                major: 8,
                minor: 1,
                patch: 2,
            }
        );
        assert_eq!(
            ordinary_context
                .get_client_name_by_id(42)
                .expect("configured client name should be preserved")
                .as_slice(),
            b"alice"
        );
        let client_info = ordinary_context
            .get_client_info_by_id(42)
            .expect("configured client info should be preserved");
        assert_eq!((client_info.id, client_info.port), (42, 6379));
    }

    #[test]
    fn with_lock_exposes_configured_context_to_closure() {
        let mut context = ThreadSafeContext::test();
        context.expect_call("INCR", &["counter"], ValkeyValue::Integer(1));

        let reply = context.with_lock(|guard| guard.call("INCR", &["counter"]));

        assert!(matches!(reply, Ok(ValkeyValue::Integer(1))));
    }

    #[test]
    fn configured_call_expectation_is_replayed_for_each_lock() {
        let mut context = ThreadSafeContext::test();
        context.expect_call("INCR", &["counter"], ValkeyValue::Integer(1));

        for _ in 0..2 {
            assert_eq!(
                context
                    .with_lock(|guard| guard.call("INCR", &["counter"]))
                    .expect("each lock should receive the configured call expectation"),
                ValkeyValue::Integer(1)
            );
        }
    }

    #[test]
    fn with_lock_releases_the_guard_when_the_closure_panics() {
        let mut context = ThreadSafeContext::test();
        context.expect_call("INCR", &["counter"], ValkeyValue::Integer(1));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            context.with_lock(|_| panic!("test panic"));
        }));

        assert!(result.is_err());
        assert!(LIVE_GUARD_CONTEXTS.with(|contexts| contexts.borrow().is_empty()));
        assert!(matches!(
            context.with_lock(|guard| guard.call("INCR", &["counter"])),
            Ok(ValkeyValue::Integer(1))
        ));
    }

    #[test]
    fn thread_safe_context_replays_bulk_valkey_string_call_reply() {
        let mut context = ThreadSafeContext::test();
        context.expect_call(
            "ECHO",
            &[] as &[&str],
            ValkeyValue::BulkValkeyString(crate::ValkeyString::test("value")),
        );

        let reply = context.with_lock(|guard| guard.call("ECHO", &[] as &[&str]));

        assert_eq!(
            reply.expect("configured call reply should be returned"),
            ValkeyValue::SimpleString("value".to_owned())
        );
    }

    #[test]
    fn thread_safe_context_can_be_reused_sequentially_across_threads() {
        let mut context = ThreadSafeContext::test();
        context
            .expect_get_client_id(42)
            .expect_call("INCR", &["counter"], ValkeyValue::Integer(1));
        let context = Arc::new(context);

        for _ in 0..2 {
            let context = Arc::clone(&context);
            std::thread::spawn(move || assert_configured_fixture(&context))
                .join()
                .expect("test thread should complete");
        }

        assert_configured_fixture(&context);
    }

    #[test]
    fn concurrent_locks_create_and_clean_up_independent_guards() {
        let mut context = ThreadSafeContext::test();
        context
            .expect_get_client_id(42)
            .expect_call("INCR", &["counter"], ValkeyValue::Integer(1));
        let context = Arc::new(context);
        let barrier = Arc::new(Barrier::new(2));

        let workers = (0..2)
            .map(|_| {
                let context = Arc::clone(&context);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let guard = context.lock();
                    let guard_ctx = guard.ctx;
                    assert!(guard_is_live(guard_ctx));

                    barrier.wait();
                    assert_eq!(guard.get_client_id(), 42);
                    assert_eq!(
                        guard
                            .call("INCR", &["counter"])
                            .expect("each concurrent guard should receive the fixture template"),
                        ValkeyValue::Integer(1)
                    );

                    drop(guard);
                    assert!(!guard_is_live(guard_ctx));
                    guard_ctx as usize
                })
            })
            .collect::<Vec<_>>();

        let guard_contexts = workers
            .into_iter()
            .map(|worker| worker.join().expect("test thread should complete"))
            .collect::<Vec<_>>();

        assert_ne!(guard_contexts[0], guard_contexts[1]);
    }

    #[test]
    fn thread_safe_context_matches_test_context_call_errors() {
        let ordinary = Context::test();
        let thread_safe = ThreadSafeContext::test();
        let ordinary_unconfigured = ordinary
            .call("MISSING", &["argument"])
            .expect_err("ordinary context should reject an unconfigured call")
            .to_string();
        let thread_safe_unconfigured = thread_safe
            .with_lock(|guard| guard.call("MISSING", &["argument"]))
            .expect_err("thread-safe context should reject an unconfigured call")
            .to_string();
        assert_eq!(thread_safe_unconfigured, ordinary_unconfigured);

        let expected = b"\0\xff".as_slice();
        let actual = b"\0\xfe".as_slice();
        let mut ordinary = Context::test();
        ordinary.expect_call("ECHO", &[expected], ValkeyValue::Integer(1));
        let mut thread_safe = ThreadSafeContext::test();
        thread_safe.expect_call("ECHO", &[expected], ValkeyValue::Integer(1));
        let ordinary_mismatch = ordinary
            .call("ECHO", &[actual])
            .expect_err("ordinary context should reject a mismatched binary argument")
            .to_string();
        let thread_safe_mismatch = thread_safe
            .with_lock(|guard| guard.call("ECHO", &[actual]))
            .expect_err("thread-safe context should reject a mismatched binary argument")
            .to_string();

        assert_eq!(thread_safe_mismatch, ordinary_mismatch);
    }

    fn assert_configured_fixture(context: &TestThreadSafeContext) {
        let guard = context.lock();
        assert_eq!(guard.get_client_id(), 42);
        assert_eq!(
            guard
                .call("INCR", &["counter"])
                .expect("configured call should be replayed on every thread"),
            ValkeyValue::Integer(1)
        );
    }

    fn guard_is_live(ctx: *mut raw::RedisModuleCtx) -> bool {
        LIVE_GUARD_CONTEXTS.with(|contexts| contexts.borrow().contains_key(&(ctx as usize)))
    }

    fn assert_invalid_lock_is_rejected(ctx: *mut raw::RedisModuleCtx) {
        thread_safe_context_lock(ctx);
        let guard = get_thread_safe_context(null_mut());

        thread_safe_context_unlock(guard);
        if !guard.is_null() {
            free_thread_safe_context(guard);
        }
        assert!(guard.is_null());
    }
}
