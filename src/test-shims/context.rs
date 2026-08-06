use crate::{raw, Context, RedisModuleClientInfo, ValkeyString};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicI32, Ordering};

const DEFAULT_CLIENT_ID: u64 = 1;

static SERVER_VERSION: AtomicI32 = AtomicI32::new(0);

thread_local! {
    static CLIENT_INFO_BY_ID: RefCell<HashMap<u64, RedisModuleClientInfo>> = RefCell::default();
}

impl Context {
    /// Creates a test context whose API return values can be configured with `expect_*` methods.
    #[must_use]
    pub fn test() -> TestContext {
        TestContext::new()
    }
}

struct ContextData {
    client_id: u64,
    client_name: Option<Vec<u8>>,
    client_username: Option<Vec<u8>>,
    current_user: Option<Vec<u8>>,
    acl_user: Option<Vec<u8>>,
    deauthentication_expected: bool,
}

impl Default for ContextData {
    fn default() -> Self {
        Self {
            client_id: DEFAULT_CLIENT_ID,
            client_name: None,
            client_username: None,
            current_user: None,
            acl_user: None,
            deauthentication_expected: false,
        }
    }
}

/// Owns a test-only [`Context`] that can be used without a running Valkey server.
pub struct TestContext {
    context: Context,
    data: Box<ContextData>,
}

impl TestContext {
    fn new() -> Self {
        super::setup_test_shims();

        let mut data = Box::new(ContextData::default());

        // GetClientInfoById has no context parameter, so its shim uses per-thread state.
        // Reset that state before each test context to prevent stale expectations leaking.
        CLIENT_INFO_BY_ID.with(|client_info_by_id| client_info_by_id.borrow_mut().clear());

        let ctx = (data.as_mut() as *mut ContextData).cast::<raw::RedisModuleCtx>();

        Self {
            context: Context::new(ctx),
            data,
        }
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
        self.data.client_name = Some(client_name.into());
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
        SERVER_VERSION.store(version, Ordering::Relaxed);
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
}

impl Deref for TestContext {
    type Target = Context;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

pub(super) extern "C" fn get_client_id(ctx: *mut raw::RedisModuleCtx) -> libc::c_ulonglong {
    if ctx.is_null() {
        return DEFAULT_CLIENT_ID;
    }

    // SAFETY: `TestContext::new` stores a live `ContextData` allocation at this opaque pointer.
    unsafe { &*ctx.cast::<ContextData>() }.client_id
}

pub(super) extern "C" fn get_client_name_by_id(
    ctx: *mut raw::RedisModuleCtx,
    client_id: libc::c_ulonglong,
) -> *mut raw::RedisModuleString {
    if ctx.is_null() {
        return null_mut();
    }

    // SAFETY: `TestContext::new` stores a live `ContextData` allocation at this opaque pointer.
    let data = unsafe { &*ctx.cast::<ContextData>() };
    if data.client_id != client_id {
        return null_mut();
    }
    let Some(client_name) = data.client_name.as_ref() else {
        return null_mut();
    };

    ValkeyString::test(client_name.clone()).take()
}

pub(super) extern "C" fn get_client_username_by_id(
    ctx: *mut raw::RedisModuleCtx,
    client_id: libc::c_ulonglong,
) -> *mut raw::RedisModuleString {
    if ctx.is_null() {
        return null_mut();
    }

    // SAFETY: `TestContext::new` stores a live `ContextData` allocation at this opaque pointer.
    let data = unsafe { &*ctx.cast::<ContextData>() };
    if data.client_id != client_id {
        return null_mut();
    }
    let Some(client_username) = data.client_username.as_ref() else {
        return null_mut();
    };

    ValkeyString::test(client_username.clone()).take()
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

    // SAFETY: Valkey supplies a writable `RedisModuleClientInfo` output buffer.
    unsafe {
        output.cast::<RedisModuleClientInfo>().write(client_info);
    }
    raw::Status::Ok as libc::c_int
}

pub(super) extern "C" fn get_current_user_name(
    ctx: *mut raw::RedisModuleCtx,
) -> *mut raw::RedisModuleString {
    if ctx.is_null() {
        return null_mut();
    }

    // SAFETY: `TestContext::new` stores a live `ContextData` allocation at this opaque pointer.
    let data = unsafe { &*ctx.cast::<ContextData>() };
    let Some(current_user) = data.current_user.as_ref() else {
        return null_mut();
    };

    ValkeyString::test(current_user.clone()).take()
}

pub(super) extern "C" fn deauthenticate_and_close_client(
    ctx: *mut raw::RedisModuleCtx,
    client_id: libc::c_ulonglong,
) -> libc::c_int {
    if ctx.is_null() {
        return raw::Status::Err as libc::c_int;
    }

    // SAFETY: `TestContext::new` stores a live `ContextData` allocation at this opaque pointer.
    let data = unsafe { &*ctx.cast::<ContextData>() };
    if data.deauthentication_expected && data.client_id == client_id {
        raw::Status::Ok as libc::c_int
    } else {
        raw::Status::Err as libc::c_int
    }
}

/// Accepts module options in tests without applying server-wide behavior.
pub(super) extern "C" fn set_module_options(_ctx: *mut raw::RedisModuleCtx, _options: libc::c_int) {
}

pub(super) extern "C" fn get_server_version() -> libc::c_int {
    SERVER_VERSION.load(Ordering::Relaxed)
}

pub(super) extern "C" fn authenticate_client_with_acl_user(
    ctx: *mut raw::RedisModuleCtx,
    name: *const libc::c_char,
    len: usize,
    _callback: raw::RedisModuleUserChangedFunc,
    _privdata: *mut libc::c_void,
    client_id: *mut u64,
) -> libc::c_int {
    if ctx.is_null() || name.is_null() {
        return raw::Status::Err as libc::c_int;
    }

    // SAFETY: `TestContext::new` stores a live `ContextData` allocation at this opaque pointer.
    let data = unsafe { &*ctx.cast::<ContextData>() };
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
            Err(crate::ValkeyError::Str("Client/Client name is null"))
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
            Err(crate::ValkeyError::Str("Client/Username is null"))
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
    fn authenticates_configured_acl_user() {
        let mut context = Context::test();
        context.expect_authenticate_client_with_acl_user("alice");
        let username = context.create_string("alice");

        assert_eq!(
            context.authenticate_client_with_acl_user(&username),
            raw::Status::Ok
        );
    }

    #[test]
    fn rejects_unconfigured_or_mismatched_acl_user() {
        let context = Context::test();
        let username = context.create_string("alice");

        assert_eq!(
            context.authenticate_client_with_acl_user(&username),
            raw::Status::Err
        );

        let mut context = Context::test();
        context.expect_authenticate_client_with_acl_user("bob");

        assert_eq!(
            context.authenticate_client_with_acl_user(&username),
            raw::Status::Err
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

        assert_eq!(status, raw::Status::Ok as libc::c_int);
        assert_eq!(client_id, 42);
    }

    #[test]
    fn rejects_acl_authentication_with_null_context_or_name() {
        assert_eq!(
            authenticate_client_with_acl_user(
                null_mut(),
                std::ptr::null(),
                0,
                None,
                null_mut(),
                null_mut(),
            ),
            raw::Status::Err as libc::c_int
        );

        let context = Context::test();
        assert_eq!(
            authenticate_client_with_acl_user(
                context.get_raw(),
                std::ptr::null(),
                0,
                None,
                null_mut(),
                null_mut(),
            ),
            raw::Status::Err as libc::c_int
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
            raw::Status::Ok
        );
    }

    #[test]
    fn deauthenticates_current_client() {
        let mut context = Context::test();
        context.expect_deauthenticate_and_close_client_by_id(42);

        assert_eq!(context.deauthenticate_and_close_client(), raw::Status::Ok);
    }

    #[test]
    fn rejects_unconfigured_client_deauthentication() {
        let mut context = Context::test();
        context.expect_deauthenticate_and_close_client_by_id(42);

        assert_eq!(
            context.deauthenticate_and_close_client_by_id(7),
            raw::Status::Err
        );
    }

    #[test]
    fn requires_deauthentication_expectation() {
        let context = Context::test();

        assert_eq!(
            context.deauthenticate_and_close_client_by_id(DEFAULT_CLIENT_ID),
            raw::Status::Err
        );
    }

    #[test]
    fn rejects_deauthentication_with_null_context() {
        assert_eq!(
            deauthenticate_and_close_client(null_mut(), 42),
            raw::Status::Err as libc::c_int
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
            .get_client_info()
            .expect("configured client info should be returned");

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
            Err(crate::ValkeyError::Str("Client/Info not found"))
        ));
    }

    #[test]
    fn get_client_info_by_id_rejects_unknown_id() {
        let _context = Context::test();
        let mut client_info = RedisModuleClientInfo::default();

        assert_eq!(
            get_client_info_by_id((&mut client_info as *mut RedisModuleClientInfo).cast(), 42,),
            raw::Status::Err as libc::c_int
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
}
