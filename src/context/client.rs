use crate::{
    Context, RedisModuleClientInfo, RedisModule_DeauthenticateAndCloseClient,
    RedisModule_GetClientCertificate, RedisModule_GetClientId, RedisModule_GetClientInfoById,
    RedisModule_GetClientNameById, RedisModule_GetClientUserNameById,
    RedisModule_SetClientNameById, Status, ValkeyError, ValkeyResult, ValkeyString, ValkeyValue,
    VALKEYMODULE_OK,
    Context, RedisModuleClientInfo, RedisModule_DeauthenticateAndCloseClient,
    RedisModule_GetClientCertificate, RedisModule_GetClientId, RedisModule_GetClientInfoById,
    RedisModule_GetClientNameById, RedisModule_GetClientUserNameById,
    RedisModule_SetClientNameById, Status, ValkeyError, ValkeyResult, ValkeyString, ValkeyValue,
    VALKEYMODULE_OK,
};
use std::ffi::CStr;
use std::os::raw::c_void;

impl RedisModuleClientInfo {
    fn new() -> Self {
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
        let mut mci = RedisModuleClientInfo::new();
        let mci_ptr: *mut c_void = &mut mci as *mut _ as *mut c_void;
        unsafe {
            RedisModule_GetClientInfoById.unwrap()(mci_ptr, client_id);
        };
        if mci_ptr.is_null() {
            Err(ValkeyError::Str("Client/Info is null"))
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
