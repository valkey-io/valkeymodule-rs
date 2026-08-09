use crate::{
    Context, RedisModuleClientInfo, RedisModule_DeauthenticateAndCloseClient,
    RedisModule_GetClientCertificate, RedisModule_GetClientId, RedisModule_GetClientInfoById,
    RedisModule_GetClientNameById, RedisModule_GetClientUserNameById,
    RedisModule_SetClientNameById, Status, ValkeyError, ValkeyResult, ValkeyString, ValkeyValue,
};
use std::ffi::CStr;
use std::os::raw::c_void;

impl Default for RedisModuleClientInfo {
    fn default() -> Self {
        Self {
            version: 1,
            flags: 0,
            id: 0,
            addr: [0; 46],
            port: 0,
            db: 0,
        }
    }
}

/// GetClientNameById, GetClientUserNameById and GetClientCertificate use autoMemoryAdd on the ValkeyModuleString pointer
/// after the callback (command, server event handler, ...) these ValkeyModuleString pointers will be freed automatically
impl Context {
    pub fn get_client_id(&self) -> u64 {
        unsafe { RedisModule_GetClientId.unwrap()(self.ctx) }
    }

    /// wrapper for RedisModule_GetClientNameById
    pub fn get_client_name_by_id(&self, client_id: u64) -> ValkeyResult<ValkeyString> {
        let client_name = unsafe { RedisModule_GetClientNameById.unwrap()(self.ctx, client_id) };
        if client_name.is_null() {
            Err(ValkeyError::Str("Client/Client name is null"))
        } else {
            Ok(ValkeyString::from_redis_module_string(
                self.ctx,
                client_name,
            ))
        }
    }

    /// wrapper for RedisModule_GetClientNameById using current client ID
    pub fn get_client_name(&self) -> ValkeyResult<ValkeyString> {
        self.get_client_name_by_id(self.get_client_id())
    }

    /// wrapper for RedisModule_SetClientNameById
    pub fn set_client_name_by_id(&self, client_id: u64, client_name: &ValkeyString) -> Status {
        let resp = unsafe { RedisModule_SetClientNameById.unwrap()(client_id, client_name.inner) };
        Status::from(resp)
    }

    /// wrapper for RedisModule_SetClientNameById using current client ID
    pub fn set_client_name(&self, client_name: &ValkeyString) -> Status {
        self.set_client_name_by_id(self.get_client_id(), client_name)
    }

    /// wrapper for RedisModule_GetClientUserNameById
    pub fn get_client_username_by_id(&self, client_id: u64) -> ValkeyResult<ValkeyString> {
        let client_username =
            unsafe { RedisModule_GetClientUserNameById.unwrap()(self.ctx, client_id) };
        if client_username.is_null() {
            Err(ValkeyError::Str("Client/Username is null"))
        } else {
            Ok(ValkeyString::from_redis_module_string(
                self.ctx,
                client_username,
            ))
        }
    }

    /// wrapper for RedisModule_GetClientUserNameById using current client ID
    pub fn get_client_username(&self) -> ValkeyResult<ValkeyString> {
        self.get_client_username_by_id(self.get_client_id())
    }

    /// wrapper for RedisModule_GetClientCertificate
    pub fn get_client_cert(&self) -> ValkeyResult<ValkeyString> {
        let client_id = self.get_client_id();
        let client_cert = unsafe { RedisModule_GetClientCertificate.unwrap()(self.ctx, client_id) };
        if client_cert.is_null() {
            Err(ValkeyError::Str("Client/Cert is null"))
        } else {
            Ok(ValkeyString::from_redis_module_string(
                self.ctx,
                client_cert,
            ))
        }
    }

    /// wrapper for RedisModule_GetClientInfoById
    pub fn get_client_info_by_id(&self, client_id: u64) -> ValkeyResult<RedisModuleClientInfo> {
        let mut mci = RedisModuleClientInfo::default();
        let mci_ptr: *mut c_void = &mut mci as *mut _ as *mut c_void;
        let status: Status =
            unsafe { RedisModule_GetClientInfoById.unwrap()(mci_ptr, client_id).into() };
        if status != Status::Ok {
            Err(ValkeyError::Str("Client/Info not found"))
        } else {
            Ok(mci)
        }
    }

    /// wrapper for RedisModule_GetClientInfoById using current client ID
    pub fn get_client_info(&self) -> ValkeyResult<RedisModuleClientInfo> {
        self.get_client_info_by_id(self.get_client_id())
    }

    /// wrapper to get the client IP address from RedisModuleClientInfo
    pub fn get_client_ip_by_id(&self, client_id: u64) -> ValkeyResult<String> {
        let client_info = self.get_client_info_by_id(client_id)?;
        let c_str_addr = unsafe { CStr::from_ptr(client_info.addr.as_ptr()) };
        let ip_addr_as_string = c_str_addr.to_string_lossy().into_owned();
        Ok(ip_addr_as_string)
    }

    /// wrapper to get the client IP address from RedisModuleClientInfo using current client ID
    pub fn get_client_ip(&self) -> ValkeyResult<String> {
        self.get_client_ip_by_id(self.get_client_id())
    }

    pub fn deauthenticate_and_close_client_by_id(&self, client_id: u64) -> Status {
        let resp =
            unsafe { RedisModule_DeauthenticateAndCloseClient.unwrap()(self.ctx, client_id) };
        Status::from(resp)
    }

    pub fn deauthenticate_and_close_client(&self) -> Status {
        self.deauthenticate_and_close_client_by_id(self.get_client_id())
    }

    pub fn config_get(&self, config: String) -> ValkeyResult<ValkeyString> {
        match self.call("CONFIG", &["GET", &config])? {
            ValkeyValue::Array(array) if array.len() == 2 => match &array[1] {
                ValkeyValue::SimpleString(val) => Ok(ValkeyString::create(None, val.clone())),
                _ => Err(ValkeyError::Str("Config value is not a string")),
            },
            _ => Err(ValkeyError::Str("Unexpected CONFIG GET response")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CLIENT_CERTIFICATE: &str = concat!(
        "-----BEGIN CERTIFICATE-----\n",
        "VGhpcyBpcyBhIHRlc3QgY2xpZW50IGNlcnRpZmljYXRlLg==\n",
        "-----END CERTIFICATE-----\n"
    );

    #[test]
    fn returns_current_client_id() {
        let mut context = Context::test();
        context.expect_get_client_id(42);

        assert_eq!(context.get_client_id(), 42);
    }

    #[test]
    fn gets_client_name_and_rejects_unknown_client_id() {
        let mut context = Context::test();
        context.expect_get_client_name_by_id(42, "alice");

        assert_eq!(
            context
                .get_client_name()
                .expect("configured client name should be returned")
                .as_slice(),
            b"alice"
        );
        assert!(matches!(
            context.get_client_name_by_id(7),
            Err(ValkeyError::Str("Client/Client name is null"))
        ));
    }

    #[test]
    fn gets_client_username_and_rejects_unknown_client_id() {
        let mut context = Context::test();
        context.expect_get_client_username_by_id(42, "alice");

        assert_eq!(
            context
                .get_client_username()
                .expect("configured client username should be returned")
                .as_slice(),
            b"alice"
        );
        assert!(matches!(
            context.get_client_username_by_id(7),
            Err(ValkeyError::Str("Client/Username is null"))
        ));
    }

    #[test]
    fn deauthenticates_configured_client_id_and_rejects_unknown_client_id() {
        let mut context = Context::test();
        context.expect_deauthenticate_and_close_client_by_id(42);

        assert_eq!(context.deauthenticate_and_close_client(), Status::Ok);
        assert_eq!(
            context.deauthenticate_and_close_client_by_id(42),
            Status::Ok
        );
        assert_eq!(
            context.deauthenticate_and_close_client_by_id(7),
            Status::Err
        );
    }

    #[test]
    fn gets_configured_client_info_and_rejects_unknown_client_id() {
        let mut context = Context::test();
        let client_info = RedisModuleClientInfo {
            id: 42,
            addr: [1; 46],
            port: 6379,
            db: 2,
            ..RedisModuleClientInfo::default()
        };
        context.expect_get_client_info_by_id(client_info);

        assert!(context.get_client_info().is_ok());
        assert!(context.get_client_info_by_id(42).is_ok());
        assert!(matches!(
            context.get_client_info_by_id(7),
            Err(ValkeyError::Str("Client/Info not found"))
        ));
    }

    #[test]
    fn client_info_default_uses_version_one_and_zeroed_fields() {
        let client_info = RedisModuleClientInfo::default();

        assert_eq!(client_info.version, 1);
        assert_eq!(client_info.flags, 0);
        assert_eq!(client_info.id, 0);
        assert_eq!(client_info.addr, [0; 46]);
        assert_eq!(client_info.port, 0);
        assert_eq!(client_info.db, 0);
    }

    #[test]
    fn gets_configured_client_ip_and_rejects_unknown_client_id() {
        let mut context = Context::test();
        context.expect_get_client_ip_by_id(42, "127.0.0.1");

        assert_eq!(
            context
                .get_client_ip()
                .expect("configured client IP should be returned"),
            "127.0.0.1"
        );
        assert!(matches!(
            context.get_client_ip_by_id(7),
            Err(ValkeyError::Str("Client/Info not found"))
        ));

        context.expect_get_client_ip_by_id(42, "2001:db8::1");

        assert_eq!(
            context
                .get_client_ip()
                .expect("configured IPv6 client IP should be returned"),
            "2001:db8::1"
        );
    }

    #[test]
    fn gets_configured_client_certificate_and_rejects_missing_certificate() {
        let mut context = Context::test();
        context.expect_get_client_cert(TEST_CLIENT_CERTIFICATE);

        assert_eq!(
            context
                .get_client_cert()
                .expect("configured client certificate should be returned")
                .as_slice(),
            TEST_CLIENT_CERTIFICATE.as_bytes()
        );

        let context = Context::test();
        assert!(matches!(
            context.get_client_cert(),
            Err(ValkeyError::Str("Client/Cert is null"))
        ));
    }

    #[test]
    fn sets_current_client_name_and_rejects_unknown_client_id() {
        let mut context = Context::test();
        context.expect_set_client_name_by_id(42);
        let client_name = context.create_string("bob");

        assert_eq!(context.set_client_name(&client_name), Status::Ok);
        assert_eq!(context.set_client_name_by_id(7, &client_name), Status::Err);
    }

    #[test]
    fn gets_configured_config_value() {
        let mut context = Context::test();
        context.expect_call(
            "CONFIG",
            &["GET", "hz"],
            ValkeyValue::Array(vec![
                ValkeyValue::SimpleString("hz".to_owned()),
                ValkeyValue::SimpleString("10".to_owned()),
            ]),
        );

        assert_eq!(
            context
                .config_get("hz".to_owned())
                .expect("configured config value should be returned")
                .as_slice(),
            b"10"
        );
    }

    #[test]
    fn rejects_unconfigured_test_call() {
        let context = Context::test();

        assert!(matches!(
            context.config_get("hz".to_owned()),
            Err(ValkeyError::String(message)) if message == "unexpected call: CONFIG GET hz"
        ));
    }
}
