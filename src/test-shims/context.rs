use super::call::{TestCallExpectation, TestCallReply};
use crate::{raw, Context, RedisModuleClientInfo, ValkeyString, ValkeyValue};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::ptr::null_mut;

pub(super) fn install() {
    // SAFETY: `setup_test_shims` calls this once after verifying the real API is uninitialized.
    unsafe {
        raw::RedisModule_GetClientId = Some(get_client_id);
        raw::RedisModule_GetClientNameById = Some(get_client_name_by_id);
        raw::RedisModule_SetClientNameById = Some(set_client_name_by_id);
        raw::RedisModule_GetClientUserNameById = Some(get_client_username_by_id);
        raw::RedisModule_GetClientCertificate = Some(get_client_certificate);
        raw::RedisModule_GetClientInfoById = Some(get_client_info_by_id);
        raw::RedisModule_GetCurrentUserName = Some(get_current_user_name);
        raw::RedisModule_DeauthenticateAndCloseClient = Some(deauthenticate_and_close_client);
        raw::RedisModule_SetModuleOptions = Some(set_module_options);
        raw::RedisModule_GetServerVersion = Some(get_server_version);
        raw::RedisModule_AuthenticateClientWithACLUser = Some(authenticate_client_with_acl_user);
    }
}

const DEFAULT_CLIENT_ID: u64 = 1;

// Stores expectations for APIs that cannot keep all state in `ContextData` and tracks live context
// addresses for pointer validation. Thread-local storage isolates concurrently running tests.
thread_local! {
    static CLIENT_INFO_BY_ID: RefCell<HashMap<u64, RedisModuleClientInfo>> = RefCell::default();
    static CLIENT_NAME_BY_ID: RefCell<HashMap<u64, Vec<u8>>> = RefCell::default();
    static SERVER_VERSION: Cell<libc::c_int> = Cell::new(0);
    static TEST_CONTEXTS: RefCell<HashSet<usize>> = RefCell::default();
}

impl Context {
    /// Creates a test context whose API return values can be configured with `expect_*` methods.
    #[must_use]
    pub fn test() -> TestContext {
        TestContext::new()
    }
}

/// Stores expectations and mutable state owned by one test context.
struct ContextData {
    client_id: u64,
    client_username: Option<Vec<u8>>,
    client_cert: Option<Vec<u8>>,
    current_user: Option<Vec<u8>>,
    acl_user: Option<Vec<u8>>,
    deauthentication_expected: bool,
    call_expectations: Vec<TestCallExpectation>,
}

// Establishes the baseline state used by a newly created test context.
impl Default for ContextData {
    fn default() -> Self {
        Self {
            client_id: DEFAULT_CLIENT_ID,
            client_username: None,
            client_cert: None,
            current_user: None,
            acl_user: None,
            deauthentication_expected: false,
            call_expectations: Vec::new(),
        }
    }
}

/// Owns a test-only [`Context`] that can be used without a running Valkey server.
pub struct TestContext {
    context: Context,
    data: Box<ContextData>,
}

// Constructs test contexts and configures their callback expectations.
impl TestContext {
    fn new() -> Self {
        super::setup_test_shims();
        Self::reset_thread_local_expectations();
        Self::new_registered()
    }

    /// Creates a guard context without resetting expectations owned by its locking thread.
    pub(super) fn new_thread_safe_guard() -> Self {
        super::setup_test_shims();
        Self::new_registered()
    }

    /// Allocates context data and registers its opaque pointer on the current thread.
    fn new_registered() -> Self {
        let mut data = Box::new(ContextData::default());
        let ctx = (data.as_mut() as *mut ContextData).cast::<raw::RedisModuleCtx>();

        // Register the backing allocation before callbacks can receive its opaque context pointer.
        TEST_CONTEXTS.with(|test_contexts| {
            test_contexts.borrow_mut().insert(ctx as usize);
        });

        Self {
            context: Context::new(ctx),
            data,
        }
    }

    /// Clears expectations shared by top-level test contexts on the current thread.
    fn reset_thread_local_expectations() {
        // GetClientInfoById has no context parameter, so its shim uses per-thread state.
        // Reset that state before each test context to prevent stale expectations leaking.
        CLIENT_INFO_BY_ID.with(|client_info_by_id| client_info_by_id.borrow_mut().clear());
        // CLIENT_NAME_BY_ID outlives TestContext, so clear names configured by earlier tests.
        CLIENT_NAME_BY_ID.with(|client_name_by_id| client_name_by_id.borrow_mut().clear());
        // GetServerVersion has no context parameter, so reset its per-thread expectation too.
        SERVER_VERSION.with(|server_version| server_version.set(0));
    }

    /// Configures the value returned by [`Context::get_client_id`].
    pub fn expect_get_client_id(&mut self, client_id: u64) -> &mut Self {
        self.data.client_id = client_id;
        self
    }

    /// Configures the current client ID and the name returned for that ID.
    pub fn expect_get_client_name_by_id<T: Into<Vec<u8>>>(
        &mut self,
        client_id: u64,
        client_name: T,
    ) -> &mut Self {
        self.data.client_id = client_id;
        CLIENT_NAME_BY_ID.with(|client_name_by_id| {
            client_name_by_id
                .borrow_mut()
                .insert(client_id, client_name.into());
        });
        self
    }

    /// Configures a client ID accepted by [`Context::set_client_name_by_id`].
    pub fn expect_set_client_name_by_id(&mut self, client_id: u64) -> &mut Self {
        self.data.client_id = client_id;
        CLIENT_NAME_BY_ID.with(|client_name_by_id| {
            client_name_by_id.borrow_mut().entry(client_id).or_default();
        });
        self
    }

    /// Configures the current client ID and the username returned for that ID.
    pub fn expect_get_client_username_by_id<T: Into<Vec<u8>>>(
        &mut self,
        client_id: u64,
        client_username: T,
    ) -> &mut Self {
        self.data.client_id = client_id;
        self.data.client_username = Some(client_username.into());
        self
    }

    /// Configures the certificate returned for the current client.
    pub fn expect_get_client_cert<T: Into<Vec<u8>>>(&mut self, client_cert: T) -> &mut Self {
        self.data.client_cert = Some(client_cert.into());
        self
    }

    /// Configures the client information returned for a client ID.
    pub fn expect_get_client_info_by_id(
        &mut self,
        client_info: RedisModuleClientInfo,
    ) -> &mut Self {
        let client_id = client_info.id;
        self.data.client_id = client_id;
        CLIENT_INFO_BY_ID.with(|client_info_by_id| {
            client_info_by_id
                .borrow_mut()
                .insert(client_id, client_info);
        });
        self
    }

    /// Configures the client IP address returned for a client ID.
    pub fn expect_get_client_ip_by_id(&mut self, client_id: u64, client_ip: &str) -> &mut Self {
        assert!(
            client_ip.len() < 46,
            "client IP must fit in RedisModuleClientInfo::addr"
        );

        let mut client_info = RedisModuleClientInfo {
            id: client_id,
            ..RedisModuleClientInfo::default()
        };
        for (address_byte, ip_byte) in client_info.addr.iter_mut().zip(client_ip.bytes()) {
            *address_byte = ip_byte as libc::c_char;
        }

        self.expect_get_client_info_by_id(client_info)
    }

    /// Configures the username returned by [`Context::get_current_user`].
    pub fn expect_get_current_user<T: Into<Vec<u8>>>(&mut self, current_user: T) -> &mut Self {
        self.data.current_user = Some(current_user.into());
        self
    }

    /// Configures the server version returned by the test shim.
    pub fn expect_get_server_version(&mut self, major: u8, minor: u8, patch: u8) -> &mut Self {
        let version = (libc::c_int::from(major) << 16)
            | (libc::c_int::from(minor) << 8)
            | libc::c_int::from(patch);
        SERVER_VERSION.with(|server_version| server_version.set(version));
        self
    }

    /// Configures the ACL username accepted by [`Context::authenticate_client_with_acl_user`].
    pub fn expect_authenticate_client_with_acl_user<T: Into<Vec<u8>>>(
        &mut self,
        username: T,
    ) -> &mut Self {
        self.data.acl_user = Some(username.into());
        self
    }

    /// Configures the client ID accepted by [`Context::deauthenticate_and_close_client_by_id`].
    pub fn expect_deauthenticate_and_close_client_by_id(&mut self, client_id: u64) -> &mut Self {
        self.data.client_id = client_id;
        self.data.deauthentication_expected = true;
        self
    }

    /// Configures the value returned by [`Context::config_get`] for a configuration name.
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

    /// Configures a reply returned by [`Context::call`] for an exact command and argument list.
    pub fn expect_call<T: AsRef<[u8]>>(
        &mut self,
        command: impl AsRef<[u8]>,
        args: &[T],
        reply: ValkeyValue,
    ) -> &mut Self {
        let expectation = TestCallExpectation::new(command, args, reply)
            .expect("unsupported reply type configured for test-shim call");
        self.expect_call_reply(expectation)
    }

    /// Adds an already normalized expectation for use by another test fixture.
    pub(super) fn expect_call_reply(&mut self, expectation: TestCallExpectation) -> &mut Self {
        self.data.call_expectations.push(expectation);
        self
    }
}

impl Deref for TestContext {
    type Target = Context;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

// Removes the context address when the backing test data is about to be released.
impl Drop for TestContext {
    fn drop(&mut self) {
        // Unregister the address before the backing allocation is released.
        TEST_CONTEXTS.with(|test_contexts| {
            test_contexts
                .borrow_mut()
                .remove(&(self.context.ctx as usize));
        });
    }
}

/// Returns a test-shim reply when `ctx` belongs to a live [`TestContext`].
///
/// A matching expectation returns its configured reply; an unexpected call returns an error
/// reply. `None` lets ordinary contexts continue to the real Valkey API.
pub(crate) fn try_call(
    ctx: *mut raw::RedisModuleCtx,
    command: &str,
    args: &[*mut raw::RedisModuleString],
) -> Option<*mut raw::RedisModuleCallReply> {
    with_data_mut(ctx, |data| {
        let args = args
            .iter()
            .map(|arg| {
                // SAFETY: call arguments for a test context are allocated by the string test shim.
                unsafe { super::valkey_string::string_data(*arg) }.to_vec()
            })
            .collect::<Vec<_>>();
        let reply = data
            .call_expectations
            .iter()
            .find(|expectation| {
                expectation.command == command.as_bytes() && expectation.args == args
            })
            .map(|expectation| expectation.reply.clone());

        let reply = match reply {
            Some(reply) => reply,
            None => TestCallReply::error(format!(
                "unexpected call: {command} {}",
                args.iter()
                    .map(|arg| String::from_utf8_lossy(arg))
                    .collect::<Vec<_>>()
                    .join(" ")
            )),
        };
        reply.into_raw()
    })
}

pub(super) extern "C" fn get_client_id(ctx: *mut raw::RedisModuleCtx) -> libc::c_ulonglong {
    with_data(ctx, |data| data.client_id).unwrap_or(DEFAULT_CLIENT_ID)
}

pub(super) extern "C" fn get_client_name_by_id(
    ctx: *mut raw::RedisModuleCtx,
    client_id: libc::c_ulonglong,
) -> *mut raw::RedisModuleString {
    with_data(ctx, |_| {
        let client_name = CLIENT_NAME_BY_ID
            .with(|client_name_by_id| client_name_by_id.borrow().get(&client_id).cloned());
        let Some(client_name) = client_name else {
            return null_mut();
        };

        ValkeyString::test(client_name).take()
    })
    .unwrap_or(null_mut())
}

pub(super) extern "C" fn set_client_name_by_id(
    client_id: libc::c_ulonglong,
    client_name: *mut raw::RedisModuleString,
) -> libc::c_int {
    if client_name.is_null() {
        return raw::Status::Err as libc::c_int;
    }

    // SAFETY: The caller supplies a live module string allocated by the string shim.
    let client_name = unsafe { super::valkey_string::string_data(client_name) }.to_vec();
    let updated = CLIENT_NAME_BY_ID.with(|client_name_by_id| {
        let mut client_name_by_id = client_name_by_id.borrow_mut();
        let Some(configured_name) = client_name_by_id.get_mut(&client_id) else {
            return false;
        };
        *configured_name = client_name;
        true
    });

    if updated {
        raw::Status::Ok as libc::c_int
    } else {
        raw::Status::Err as libc::c_int
    }
}

pub(super) extern "C" fn get_client_username_by_id(
    ctx: *mut raw::RedisModuleCtx,
    client_id: libc::c_ulonglong,
) -> *mut raw::RedisModuleString {
    with_data(ctx, |data| {
        if data.client_id != client_id {
            return null_mut();
        }
        let Some(client_username) = data.client_username.as_ref() else {
            return null_mut();
        };

        ValkeyString::test(client_username.clone()).take()
    })
    .unwrap_or(null_mut())
}

pub(super) extern "C" fn get_client_certificate(
    ctx: *mut raw::RedisModuleCtx,
    client_id: libc::c_ulonglong,
) -> *mut raw::RedisModuleString {
    with_data(ctx, |data| {
        if data.client_id != client_id {
            return null_mut();
        }
        let Some(client_cert) = data.client_cert.as_ref() else {
            return null_mut();
        };

        ValkeyString::test(client_cert.clone()).take()
    })
    .unwrap_or(null_mut())
}

pub(super) extern "C" fn get_client_info_by_id(
    output: *mut libc::c_void,
    client_id: libc::c_ulonglong,
) -> libc::c_int {
    if output.is_null() {
        return raw::Status::Err as libc::c_int;
    }

    let client_info = CLIENT_INFO_BY_ID
        .with(|client_info_by_id| client_info_by_id.borrow().get(&client_id).copied());
    let Some(client_info) = client_info else {
        return raw::Status::Err as libc::c_int;
    };

    // SAFETY: The caller supplies a writable `RedisModuleClientInfo` output buffer.
    unsafe {
        output.cast::<RedisModuleClientInfo>().write(client_info);
    }
    raw::Status::Ok as libc::c_int
}

pub(super) extern "C" fn get_current_user_name(
    ctx: *mut raw::RedisModuleCtx,
) -> *mut raw::RedisModuleString {
    with_data(ctx, |data| {
        let Some(current_user) = data.current_user.as_ref() else {
            return null_mut();
        };

        ValkeyString::test(current_user.clone()).take()
    })
    .unwrap_or(null_mut())
}

pub(super) extern "C" fn deauthenticate_and_close_client(
    ctx: *mut raw::RedisModuleCtx,
    client_id: libc::c_ulonglong,
) -> libc::c_int {
    with_data(ctx, |data| {
        if data.deauthentication_expected && data.client_id == client_id {
            raw::Status::Ok as libc::c_int
        } else {
            raw::Status::Err as libc::c_int
        }
    })
    .unwrap_or(raw::Status::Err as libc::c_int)
}

/// Accepts module options in tests without applying server-wide behavior.
pub(super) extern "C" fn set_module_options(ctx: *mut raw::RedisModuleCtx, _options: libc::c_int) {
    let _ = with_data(ctx, |_| ());
}

pub(super) extern "C" fn get_server_version() -> libc::c_int {
    SERVER_VERSION.with(Cell::get)
}

pub(super) extern "C" fn authenticate_client_with_acl_user(
    ctx: *mut raw::RedisModuleCtx,
    name: *const libc::c_char,
    len: usize,
    _callback: raw::RedisModuleUserChangedFunc,
    _privdata: *mut libc::c_void,
    client_id: *mut u64,
) -> libc::c_int {
    if name.is_null() {
        return raw::Status::Err as libc::c_int;
    }

    with_data(ctx, |data| {
        // SAFETY: The callback contract requires `name` to reference at least `len` readable bytes.
        let name = unsafe { std::slice::from_raw_parts(name.cast::<u8>(), len) };
        if data.acl_user.as_deref() != Some(name) {
            return raw::Status::Err as libc::c_int;
        }

        if !client_id.is_null() {
            // SAFETY: A non-null `client_id` points to writable storage supplied by the caller.
            unsafe {
                *client_id = data.client_id;
            }
        }

        raw::Status::Ok as libc::c_int
    })
    .unwrap_or(raw::Status::Err as libc::c_int)
}

/// Runs `operation` with shared access to a live test context's backing data.
///
/// Returns `None` when `ctx` is null, foreign, or stale.
fn with_data<T>(
    ctx: *mut raw::RedisModuleCtx,
    operation: impl FnOnce(&ContextData) -> T,
) -> Option<T> {
    if ctx.is_null()
        || !TEST_CONTEXTS.with(|test_contexts| test_contexts.borrow().contains(&(ctx as usize)))
    {
        return None;
    }

    // SAFETY: the registry contains only live `ContextData` allocations, and context callbacks
    // execute synchronously on one thread.
    Some(operation(unsafe { &*ctx.cast::<ContextData>() }))
}

/// Runs `operation` with mutable access to a live test context's backing data.
///
/// Returns `None` when `ctx` is null, foreign, or stale.
fn with_data_mut<T>(
    ctx: *mut raw::RedisModuleCtx,
    operation: impl FnOnce(&mut ContextData) -> T,
) -> Option<T> {
    if ctx.is_null()
        || !TEST_CONTEXTS.with(|test_contexts| test_contexts.borrow().contains(&(ctx as usize)))
    {
        return None;
    }

    // SAFETY: the registry contains only live, uniquely owned `ContextData` allocations, and
    // context callbacks execute synchronously on one thread.
    Some(operation(unsafe { &mut *ctx.cast::<ContextData>() }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{redisvalue::ValkeyValueKey, CallOptionsBuilder, CallResult, Status, ValkeyError};
    use std::collections::BTreeMap;
    use std::ptr::null;

    const TEST_CLIENT_CERTIFICATE: &str = concat!(
        "-----BEGIN CERTIFICATE-----\n",
        "VGhpcyBpcyBhIHRlc3QgY2xpZW50IGNlcnRpZmljYXRlLg==\n",
        "-----END CERTIFICATE-----\n"
    );

    #[test]
    fn returns_configured_client_id() {
        let mut context = Context::test();
        context.expect_get_client_id(42);

        assert_eq!(context.get_client_id(), 42);
    }

    #[test]
    fn defaults_client_id_when_no_expectation_is_configured() {
        let context = Context::test();

        assert_eq!(context.get_client_id(), 1);
    }

    #[test]
    fn replaces_configured_client_id() {
        let mut context = Context::test();
        context.expect_get_client_id(10).expect_get_client_id(20);

        assert_eq!(context.get_client_id(), 20);
    }

    #[test]
    fn returns_configured_client_name_by_id() {
        let mut context = Context::test();
        context.expect_get_client_name_by_id(42, "alice");

        let client_name = context
            .get_client_name_by_id(42)
            .expect("configured client name should be returned");

        assert_eq!(client_name.as_slice(), b"alice");
    }

    #[test]
    fn rejects_unconfigured_client_id() {
        let mut context = Context::test();
        context.expect_get_client_name_by_id(42, "alice");

        assert!(matches!(
            context.get_client_name_by_id(7),
            Err(ValkeyError::Str("Client/Client name is null"))
        ));
    }

    #[test]
    fn returns_current_client_name() {
        let mut context = Context::test();
        context.expect_get_client_name_by_id(42, "alice");

        let client_name = context
            .get_client_name()
            .expect("current client name should be returned");

        assert_eq!(client_name.as_slice(), b"alice");
    }

    #[test]
    fn returns_configured_client_username_by_id() {
        let mut context = Context::test();
        context.expect_get_client_username_by_id(42, "alice");

        let client_username = context
            .get_client_username_by_id(42)
            .expect("configured client username should be returned");

        assert_eq!(client_username.as_slice(), b"alice");
    }

    #[test]
    fn rejects_unconfigured_username_client_id() {
        let mut context = Context::test();
        context.expect_get_client_username_by_id(42, "alice");

        assert!(matches!(
            context.get_client_username_by_id(7),
            Err(ValkeyError::Str("Client/Username is null"))
        ));
    }

    #[test]
    fn returns_current_client_username() {
        let mut context = Context::test();
        context.expect_get_client_username_by_id(42, "alice");

        let client_username = context
            .get_client_username()
            .expect("current client username should be returned");

        assert_eq!(client_username.as_slice(), b"alice");
    }

    #[test]
    fn returns_configured_current_user() {
        let mut context = Context::test();
        context.expect_get_current_user("alice");

        assert_eq!(context.get_current_user().as_slice(), b"alice");
    }

    #[test]
    fn returns_configured_server_version() {
        let mut context = Context::test();
        context.expect_get_server_version(8, 1, 2);

        assert_eq!(
            context
                .get_server_version()
                .expect("configured server version should be returned"),
            raw::Version {
                major: 8,
                minor: 1,
                patch: 2,
            }
        );
    }

    #[test]
    fn new_test_context_resets_server_version() {
        let mut first_context = Context::test();
        first_context.expect_get_server_version(8, 1, 2);

        let second_context = Context::test();

        assert_eq!(
            second_context
                .get_server_version()
                .expect("default server version should be returned"),
            raw::Version {
                major: 0,
                minor: 0,
                patch: 0,
            }
        );
    }

    #[test]
    fn authenticates_configured_acl_user() {
        let mut context = Context::test();
        context.expect_authenticate_client_with_acl_user("alice");
        let username = context.create_string("alice");

        assert_eq!(
            context.authenticate_client_with_acl_user(&username),
            Status::Ok
        );
    }

    #[test]
    fn rejects_unconfigured_or_mismatched_acl_user() {
        let context = Context::test();
        let username = context.create_string("alice");

        assert_eq!(
            context.authenticate_client_with_acl_user(&username),
            Status::Err
        );

        let mut context = Context::test();
        context.expect_authenticate_client_with_acl_user("bob");

        assert_eq!(
            context.authenticate_client_with_acl_user(&username),
            Status::Err
        );
    }

    #[test]
    fn writes_client_id_for_configured_acl_user() {
        let mut context = Context::test();
        context
            .expect_get_client_id(42)
            .expect_authenticate_client_with_acl_user("alice");
        let username = context.create_string("alice");
        let mut client_id = 0;

        let status = authenticate_client_with_acl_user(
            context.get_raw(),
            username.as_ptr().cast::<libc::c_char>(),
            username.len(),
            None,
            null_mut(),
            &mut client_id,
        );

        assert_eq!(status, Status::Ok as libc::c_int);
        assert_eq!(client_id, 42);
    }

    #[test]
    fn rejects_acl_authentication_with_null_context_or_name() {
        assert_eq!(
            authenticate_client_with_acl_user(null_mut(), null(), 0, None, null_mut(), null_mut(),),
            Status::Err as libc::c_int
        );

        let context = Context::test();
        assert_eq!(
            authenticate_client_with_acl_user(
                context.get_raw(),
                null(),
                0,
                None,
                null_mut(),
                null_mut(),
            ),
            Status::Err as libc::c_int
        );
    }

    #[test]
    fn rejects_current_user_with_null_context() {
        assert!(get_current_user_name(null_mut()).is_null());
    }

    #[test]
    fn deauthenticates_configured_client_by_id() {
        let mut context = Context::test();
        context.expect_deauthenticate_and_close_client_by_id(42);

        assert_eq!(
            context.deauthenticate_and_close_client_by_id(42),
            Status::Ok
        );
    }

    #[test]
    fn deauthenticates_current_client() {
        let mut context = Context::test();
        context.expect_deauthenticate_and_close_client_by_id(42);

        assert_eq!(context.deauthenticate_and_close_client(), Status::Ok);
    }

    #[test]
    fn rejects_unconfigured_client_deauthentication() {
        let mut context = Context::test();
        context.expect_deauthenticate_and_close_client_by_id(42);

        assert_eq!(
            context.deauthenticate_and_close_client_by_id(7),
            Status::Err
        );
    }

    #[test]
    fn requires_deauthentication_expectation() {
        let context = Context::test();

        assert_eq!(
            context.deauthenticate_and_close_client_by_id(DEFAULT_CLIENT_ID),
            Status::Err
        );
    }

    #[test]
    fn rejects_deauthentication_with_null_context() {
        assert_eq!(
            deauthenticate_and_close_client(null_mut(), 42),
            Status::Err as libc::c_int
        );
    }

    #[test]
    fn accepts_all_module_options() {
        let context = Context::test();
        let module_options = vec![
            raw::ModuleOptions::HANDLE_IO_ERRORS,
            raw::ModuleOptions::NO_IMPLICIT_SIGNAL_MODIFIED,
            raw::ModuleOptions::HANDLE_REPL_ASYNC_LOAD,
            raw::ModuleOptions::ALLOW_NESTED_KEYSPACE_NOTIFICATIONS,
        ];

        for options in module_options {
            context.set_module_options(options);
        }
    }

    #[test]
    fn returns_configured_client_info() {
        let mut context = Context::test();
        context.expect_get_client_info_by_id(RedisModuleClientInfo {
            version: 7,
            id: 42,
            port: 6379,
            db: 2,
            ..RedisModuleClientInfo::default()
        });

        let client_info = context
            .get_client_info_by_id(42)
            .expect("configured client info should be returned by ID");

        assert_eq!(client_info.version, 7);
        assert_eq!(client_info.id, 42);
        assert_eq!(client_info.port, 6379);
        assert_eq!(client_info.db, 2);
    }

    #[test]
    fn rejects_unconfigured_client_info() {
        let context = Context::test();

        assert!(matches!(
            context.get_client_info(),
            Err(ValkeyError::Str("Client/Info not found"))
        ));
    }

    #[test]
    fn get_client_info_by_id_rejects_unknown_id() {
        let _context = Context::test();
        let mut client_info = RedisModuleClientInfo::default();

        assert_eq!(
            get_client_info_by_id((&mut client_info as *mut RedisModuleClientInfo).cast(), 42,),
            Status::Err as libc::c_int
        );
    }

    #[test]
    fn returns_configured_client_ip() {
        let mut context = Context::test();
        context.expect_get_client_ip_by_id(42, "127.0.0.1");

        assert_eq!(
            context
                .get_client_ip()
                .expect("configured client IP should be returned"),
            "127.0.0.1"
        );
    }

    #[test]
    fn returns_configured_client_certificate() {
        let mut context = Context::test();
        context.expect_get_client_cert(TEST_CLIENT_CERTIFICATE);

        assert_eq!(
            context
                .get_client_cert()
                .expect("configured client certificate should be returned")
                .as_slice(),
            TEST_CLIENT_CERTIFICATE.as_bytes()
        );
    }

    #[test]
    fn updates_configured_client_name_and_rejects_unknown_client_id() {
        let mut context = Context::test();
        context.expect_set_client_name_by_id(42);
        let client_name = context.create_string("bob");

        assert_eq!(context.set_client_name(&client_name), Status::Ok);
        assert_eq!(
            context
                .get_client_name()
                .expect("updated client name should be returned")
                .as_slice(),
            b"bob"
        );
        assert_eq!(context.set_client_name_by_id(7, &client_name), Status::Err);
    }

    #[test]
    fn new_test_context_clears_thread_local_client_expectations() {
        let mut first_context = Context::test();
        first_context.expect_get_client_name_by_id(42, "alice");
        first_context.expect_get_client_info_by_id(RedisModuleClientInfo {
            id: 42,
            ..RedisModuleClientInfo::default()
        });

        let second_context = Context::test();

        assert!(matches!(
            second_context.get_client_name_by_id(42),
            Err(ValkeyError::Str("Client/Client name is null"))
        ));
        assert!(matches!(
            second_context.get_client_info_by_id(42),
            Err(ValkeyError::Str("Client/Info not found"))
        ));
    }

    #[test]
    fn returns_each_of_multiple_configured_client_info_entries() {
        let mut context = Context::test();
        context.expect_get_client_info_by_id(RedisModuleClientInfo {
            id: 41,
            port: 6379,
            ..RedisModuleClientInfo::default()
        });
        context.expect_get_client_info_by_id(RedisModuleClientInfo {
            id: 42,
            port: 6380,
            ..RedisModuleClientInfo::default()
        });

        let first = context
            .get_client_info_by_id(41)
            .expect("first configured client info should be returned");
        let second = context
            .get_client_info_by_id(42)
            .expect("second configured client info should be returned");

        assert_eq!((first.id, first.port), (41, 6379));
        assert_eq!((second.id, second.port), (42, 6380));
    }

    #[test]
    fn client_shims_reject_null_context_or_output() {
        assert!(get_client_name_by_id(null_mut(), 42).is_null());
        assert!(get_client_username_by_id(null_mut(), 42).is_null());
        assert!(get_client_certificate(null_mut(), 42).is_null());
        assert_eq!(
            get_client_info_by_id(null_mut(), 42),
            Status::Err as libc::c_int
        );
        assert_eq!(
            set_client_name_by_id(42, null_mut()),
            Status::Err as libc::c_int
        );
    }

    #[test]
    fn callbacks_reject_null_context() {
        assert_context_callbacks_reject(null_mut());
    }

    #[test]
    fn callbacks_reject_foreign_context() {
        let mut configured_context = Context::test();
        configured_context.expect_get_client_name_by_id(42, "alice");
        let mut foreign_data = Box::new(ContextData {
            client_id: 42,
            client_username: Some(b"alice".to_vec()),
            client_cert: Some(b"certificate".to_vec()),
            current_user: Some(b"alice".to_vec()),
            acl_user: Some(b"alice".to_vec()),
            deauthentication_expected: true,
            call_expectations: Vec::new(),
        });
        let foreign_ctx = (&mut *foreign_data as *mut ContextData).cast::<raw::RedisModuleCtx>();

        assert_context_callbacks_reject(foreign_ctx);
    }

    #[test]
    fn returns_configured_call_reply() {
        let mut context = Context::test();
        let expected = ValkeyValue::Array(vec![
            ValkeyValue::SimpleString("value".to_owned()),
            ValkeyValue::Integer(42),
            ValkeyValue::Bool(true),
            ValkeyValue::Float(1.5),
            ValkeyValue::BigNumber("12345678901234567890".to_owned()),
            ValkeyValue::Null,
            ValkeyValue::Array(vec![ValkeyValue::SimpleString("nested".to_owned())]),
        ]);
        context.expect_call("TEST", &["argument"], expected.clone());

        assert_eq!(
            context
                .call("TEST", &["argument"])
                .expect("configured call reply should be returned"),
            expected
        );
    }

    #[test]
    fn returns_bulk_valkey_string_call_reply() {
        let mut context = Context::test();
        context.expect_call(
            "ECHO",
            &[] as &[&str],
            ValkeyValue::BulkValkeyString(ValkeyString::test("value")),
        );

        assert_eq!(
            context
                .call("ECHO", &[] as &[&str])
                .expect("configured call reply should be returned"),
            ValkeyValue::SimpleString("value".to_owned())
        );
    }

    #[test]
    fn returns_configured_ordered_map_call_reply() {
        let mut context = Context::test();
        context.expect_call(
            "HGETALL",
            &["hash"],
            ValkeyValue::OrderedMap(BTreeMap::from([(
                ValkeyValueKey::String("field".to_owned()),
                ValkeyValue::SimpleString("value".to_owned()),
            )])),
        );

        assert_eq!(
            context
                .call("HGETALL", &["hash"])
                .expect("configured ordered map reply should be returned"),
            ValkeyValue::Map(HashMap::from([(
                ValkeyValueKey::String("field".to_owned()),
                ValkeyValue::SimpleString("value".to_owned()),
            )]))
        );
    }

    #[test]
    fn matches_configured_binary_call_arguments() {
        let mut context = Context::test();
        context.expect_call(
            "ECHO",
            &[b"\0\xff".as_slice()],
            ValkeyValue::SimpleString("matched".to_owned()),
        );

        assert_eq!(
            context
                .call("ECHO", &[b"\0\xff".as_slice()])
                .expect("configured binary argument should match"),
            ValkeyValue::SimpleString("matched".to_owned())
        );
    }

    #[test]
    fn returns_each_of_multiple_configured_call_replies() {
        let mut context = Context::test();
        context.expect_call("GET", &["first"], ValkeyValue::Integer(1));
        context.expect_call("GET", &["second"], ValkeyValue::Integer(2));

        assert_eq!(
            context
                .call("GET", &["first"])
                .expect("first configured call should return a reply"),
            ValkeyValue::Integer(1)
        );
        assert_eq!(
            context
                .call("GET", &["second"])
                .expect("second configured call should return a reply"),
            ValkeyValue::Integer(2)
        );
    }

    #[test]
    fn rejects_call_that_does_not_match_configured_arguments() {
        let mut context = Context::test();
        context.expect_call(
            "TEST",
            &["expected"],
            ValkeyValue::SimpleString("configured".to_owned()),
        );

        assert!(matches!(
            context.call("TEST", &["actual"]),
            Err(ValkeyError::String(message)) if message == "unexpected call: TEST actual"
        ));
    }

    #[test]
    fn rejects_call_that_does_not_match_configured_command() {
        let mut context = Context::test();
        context.expect_call(
            "EXPECTED",
            &["argument"],
            ValkeyValue::SimpleString("configured".to_owned()),
        );

        assert!(matches!(
            context.call("ACTUAL", &["argument"]),
            Err(ValkeyError::String(message))
                if message == "unexpected call: ACTUAL argument"
        ));
    }

    #[test]
    fn call_ext_returns_configured_reply() {
        let mut context = Context::test();
        context.expect_call(
            "ECHO",
            &["extended"],
            ValkeyValue::SimpleString("extended".to_owned()),
        );
        let options = CallOptionsBuilder::new().errors_as_replies().build();
        let reply: CallResult = context.call_ext("ECHO", &options, &["extended"]);

        assert_eq!(
            ValkeyValue::from(&reply),
            ValkeyValue::SimpleString("extended".to_owned())
        );
    }

    #[test]
    fn callbacks_reject_dropped_context() {
        let context = Context::test();
        let ctx = context.context.ctx;
        drop(context);

        assert_context_callbacks_reject(ctx);
    }

    fn assert_context_callbacks_reject(ctx: *mut raw::RedisModuleCtx) {
        let mut authenticated_client_id = 0;

        assert_eq!(get_client_id(ctx), DEFAULT_CLIENT_ID);
        assert!(get_client_name_by_id(ctx, 42).is_null());
        assert!(get_client_username_by_id(ctx, 42).is_null());
        assert!(get_client_certificate(ctx, 42).is_null());
        assert!(get_current_user_name(ctx).is_null());
        assert_eq!(
            deauthenticate_and_close_client(ctx, 42),
            Status::Err as libc::c_int
        );
        assert_eq!(
            authenticate_client_with_acl_user(
                ctx,
                b"alice".as_ptr().cast::<libc::c_char>(),
                5,
                None,
                null_mut(),
                &mut authenticated_client_id,
            ),
            Status::Err as libc::c_int
        );
        assert_eq!(authenticated_client_id, 0);
        assert!(try_call(ctx, "TEST", &[]).is_none());

        set_module_options(ctx, 0);
    }
}
