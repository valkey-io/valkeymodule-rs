use crate::{
    Context, RedisModuleClientInfo, RedisModule_GetClientCertificate, RedisModule_GetClientId,
    RedisModule_GetClientInfoById, RedisModule_GetClientNameById,
    RedisModule_GetClientUserNameById, RedisModule_SetClientNameById, Status, ValkeyError,
    ValkeyResult, ValkeyString,
};
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
            Err(ValkeyError::Str("Client name is null"))
        } else {
            Ok(ValkeyString::from_redis_module_string(
                self.ctx,
                client_name,
            ))
        }
    }

    /// wrapper for RedisModule_GetClientNameById using current client ID
    pub fn get_client_name(&self) -> ValkeyString {
        match self.get_client_name_by_id(self.get_client_id()) {
            Ok(tmp) => tmp,
            Err(_err) => self.create_string("Client name is null"),
        }
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
            Err(ValkeyError::Str("Client username is null"))
        } else {
            Ok(ValkeyString::from_redis_module_string(
                self.ctx,
                client_username,
            ))
        }
    }

    /// wrapper for RedisModule_GetClientUserNameById using current client ID
    pub fn get_client_username(&self) -> ValkeyString {
        match self.get_client_username_by_id(self.get_client_id()) {
            Ok(tmp) => tmp,
            Err(_err) => self.create_string("Client username is null"),
        }
    }

    /// wrapper for RedisModule_GetClientCertificate
    pub fn get_client_cert(&self) -> ValkeyResult<ValkeyString> {
        let client_id = self.get_client_id();
        let client_cert = unsafe { RedisModule_GetClientCertificate.unwrap()(self.ctx, client_id) };
        if client_cert.is_null() {
            Err(ValkeyError::Str("Client cert is null"))
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
        unsafe { RedisModule_GetClientInfoById.unwrap()(mci_ptr, client_id) };
        if mci_ptr.is_null() {
            Err(ValkeyError::Str("Client info is null"))
        } else {
            Ok(mci)
        }
    }

    /// wrapper for RedisModule_GetClientInfoById using current client ID
    pub fn get_client_info(&self) -> RedisModuleClientInfo {
        match self.get_client_info_by_id(self.get_client_id()) {
            Ok(tmp) => tmp,
            Err(_err) => RedisModuleClientInfo::new(),
        }
    }

    /// wrapper to get the client IP address from RedisModuleClientInfo
    pub fn get_client_ip_by_id(&self, client_id: u64) -> ValkeyResult<String> {
        let client_info = match self.get_client_info_by_id(client_id) {
            Ok(tmp) => tmp,
            Err(_err) => {
                return Err(ValkeyError::Str("Client info is null"));
            }
        };
        let addr_u8: Vec<u8> = client_info.addr.iter().map(|&x| x as u8).collect();
        let ip_addr_as_string = String::from_utf8_lossy(&addr_u8)
            // w/o trim it will be: "127.0.0.1\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00
            .trim_matches(char::from(0))
            .to_string();
        Ok(ip_addr_as_string)
    }

    /// wrapper to get the client IP address from RedisModuleClientInfo using current client ID
    pub fn get_client_ip(&self) -> String {
        match self.get_client_ip_by_id(self.get_client_id()) {
            Ok(tmp) => tmp,
            Err(_err) => "Client IP is null".to_string(),
        }
    }
}
