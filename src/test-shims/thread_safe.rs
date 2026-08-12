use super::context::TestContext;
use crate::context::thread_safe::{DetachedFromClient, ThreadSafeContext};
use crate::redisvalue::ValkeyValueKey;
use crate::{raw, Context, ValkeyValue};
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
    static PENDING_LOCK_STATE: RefCell<Option<SharedContextData>> = RefCell::default();
    /// Keeps each ordinary test context alive until its corresponding guard is freed.
    static LIVE_GUARD_CONTEXTS: RefCell<HashMap<usize, TestContext>> = RefCell::default();
}

impl ThreadSafeContext<DetachedFromClient> {
    /// Creates a configurable thread-safe context for tests without a running Valkey server.
    #[must_use]
    pub fn test() -> TestThreadSafeContext {
        TestThreadSafeContext::new()
    }
}

/// Stores expectations owned by one test thread-safe context.
#[derive(Clone, Default)]
struct ThreadSafeContextData {
    client_id: Option<u64>,
    call_expectations: Vec<CallExpectation>,
}

/// Provides synchronized ownership of context expectations across test threads.
type SharedContextData = Arc<Mutex<ThreadSafeContextData>>;

/// Matches one exact test-context command invocation to its configured reply.
#[derive(Clone)]
struct CallExpectation {
    command: Vec<u8>,
    args: Vec<Vec<u8>>,
    reply: OwnedCallReply,
}

/// Stores a thread-safe copy of each call-reply variant supported by the context shim.
#[derive(Clone)]
enum OwnedCallReply {
    String(Vec<u8>),
    Error(&'static str),
    Integer(i64),
    Bool(bool),
    Double(f64),
    BigNumber(String),
    Array(Vec<Self>),
    Map(Vec<(OwnedCallReplyKey, Self)>),
    Null,
}

/// Stores a thread-safe copy of each map-key variant supported by the context shim.
#[derive(Clone)]
enum OwnedCallReplyKey {
    Integer(i64),
    String(Vec<u8>),
    Bool(bool),
}

/// Owns a configurable test-only [`ThreadSafeContext`] backed by synchronized state.
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
    pub fn expect_call<T: AsRef<[u8]>>(
        &mut self,
        command: impl AsRef<[u8]>,
        args: &[T],
        reply: ValkeyValue,
    ) -> &mut Self {
        let reply = OwnedCallReply::from_value(reply)
            .expect("unsupported reply type configured for test-shim call");
        lock_state(&self.data)
            .call_expectations
            .push(CallExpectation {
                command: command.as_ref().to_vec(),
                args: args.iter().map(|arg| arg.as_ref().to_vec()).collect(),
                reply,
            });
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

pub(super) extern "C" fn get_thread_safe_context(
    _blocked_client: *mut raw::RedisModuleBlockedClient,
) -> *mut raw::RedisModuleCtx {
    PENDING_LOCK_STATE
        .with(|pending| pending.borrow_mut().take())
        .map_or_else(register_new, create_guard_context)
}

pub(super) extern "C" fn thread_safe_context_lock(ctx: *mut raw::RedisModuleCtx) {
    PENDING_LOCK_STATE.with(|pending| {
        *pending.borrow_mut() = state(ctx);
    });
}

pub(super) extern "C" fn thread_safe_context_unlock(_ctx: *mut raw::RedisModuleCtx) {
    PENDING_LOCK_STATE.with(|pending| {
        pending.borrow_mut().take();
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
    let mut context = Context::test();

    if let Some(client_id) = data.client_id {
        context.expect_get_client_id(client_id);
    }
    for expectation in data.call_expectations {
        context.expect_call(
            expectation.command,
            &expectation.args,
            expectation.reply.to_value(),
        );
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

impl OwnedCallReply {
    fn from_value(value: ValkeyValue) -> Result<Self, &'static str> {
        Ok(match value {
            ValkeyValue::SimpleString(value) => Self::String(value.into_bytes()),
            ValkeyValue::SimpleStringStatic(value) => Self::String(value.as_bytes().to_vec()),
            ValkeyValue::BulkString(value) => Self::String(value.into_bytes()),
            ValkeyValue::BulkValkeyString(value) => Self::String(value.as_slice().to_vec()),
            ValkeyValue::StringBuffer(value) => Self::String(value),
            ValkeyValue::Integer(value) => Self::Integer(value),
            ValkeyValue::Bool(value) => Self::Bool(value),
            ValkeyValue::Float(value) => Self::Double(value),
            ValkeyValue::BigNumber(value) => Self::BigNumber(value),
            ValkeyValue::Array(values) => Self::Array(
                values
                    .into_iter()
                    .map(Self::from_value)
                    .collect::<Result<_, _>>()?,
            ),
            ValkeyValue::Map(values) => Self::Map(Self::map_entries(values)?),
            ValkeyValue::OrderedMap(values) => Self::Map(Self::map_entries(values)?),
            ValkeyValue::Null => Self::Null,
            ValkeyValue::StaticError(value) => Self::Error(value),
            _ => return Err("test-shim calls do not support this reply type"),
        })
    }

    fn map_entries(
        values: impl IntoIterator<Item = (ValkeyValueKey, ValkeyValue)>,
    ) -> Result<Vec<(OwnedCallReplyKey, Self)>, &'static str> {
        values
            .into_iter()
            .map(|(key, value)| Ok((OwnedCallReplyKey::from_value(key), Self::from_value(value)?)))
            .collect()
    }

    fn to_value(&self) -> ValkeyValue {
        match self {
            Self::String(value) => ValkeyValue::StringBuffer(value.clone()),
            Self::Error(value) => ValkeyValue::StaticError(value),
            Self::Integer(value) => ValkeyValue::Integer(*value),
            Self::Bool(value) => ValkeyValue::Bool(*value),
            Self::Double(value) => ValkeyValue::Float(*value),
            Self::BigNumber(value) => ValkeyValue::BigNumber(value.clone()),
            Self::Array(values) => ValkeyValue::Array(values.iter().map(Self::to_value).collect()),
            Self::Map(values) => ValkeyValue::Map(
                values
                    .iter()
                    .map(|(key, value)| (key.to_value(), value.to_value()))
                    .collect(),
            ),
            Self::Null => ValkeyValue::Null,
        }
    }
}

impl OwnedCallReplyKey {
    fn from_value(value: ValkeyValueKey) -> Self {
        match value {
            ValkeyValueKey::Integer(value) => Self::Integer(value),
            ValkeyValueKey::String(value) => Self::String(value.into_bytes()),
            ValkeyValueKey::BulkValkeyString(value) => Self::String(value.as_slice().to_vec()),
            ValkeyValueKey::BulkString(value) => Self::String(value),
            ValkeyValueKey::Bool(value) => Self::Bool(value),
        }
    }

    fn to_value(&self) -> ValkeyValueKey {
        match self {
            Self::Integer(value) => ValkeyValueKey::Integer(*value),
            Self::String(value) => ValkeyValueKey::BulkString(value.clone()),
            Self::Bool(value) => ValkeyValueKey::Bool(*value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ValkeyValue;
    use std::ptr::null_mut;

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
    fn test_thread_safe_context_moves_to_another_thread() {
        let mut context = ThreadSafeContext::test();
        context.expect_call("INCR", &["counter"], ValkeyValue::Integer(1));

        std::thread::spawn(move || {
            let guard = context.lock();
            assert!(matches!(
                guard.call("INCR", &["counter"]),
                Ok(ValkeyValue::Integer(1))
            ));
        })
        .join()
        .expect("test thread should complete");
    }
}
