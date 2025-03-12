use crate::{
    Context, RedisModuleClientInfo, RedisModule_GetClientCertificate, RedisModule_GetClientId,
    RedisModule_GetClientInfoById, RedisModule_GetClientNameById,
    RedisModule_GetClientUserNameById, RedisModule_SetClientNameById, ValkeyString,
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
// after the callback (command, server event handler, ...) these ValkeyModuleString pointers will be freed automatically
impl Context {
    pub fn get_client_id(&self) -> u64 {
        unsafe { RedisModule_GetClientId.unwrap()(self.ctx) }
    }

    pub fn get_client_name(&self) -> ValkeyString {
        let client_id = self.get_client_id();
        let client_name = unsafe { RedisModule_GetClientNameById.unwrap()(self.ctx, client_id) };
        ValkeyString::from_redis_module_string(self.ctx, client_name)
    }

    pub fn set_client_name(&self, client_name: &ValkeyString) -> i64 {
        let client_id = self.get_client_id();
        let resp = unsafe { RedisModule_SetClientNameById.unwrap()(client_id, client_name.inner) };
        resp as i64
    }

    pub fn get_client_username(&self) -> ValkeyString {
        let client_id = self.get_client_id();
        let client_username =
            unsafe { RedisModule_GetClientUserNameById.unwrap()(self.ctx, client_id) };
        ValkeyString::from_redis_module_string(self.ctx, client_username)
    }

    pub fn get_client_username_by_id(&self, client_id: u64) -> ValkeyString {
        let client_username =
            unsafe { RedisModule_GetClientUserNameById.unwrap()(self.ctx, client_id) };
        ValkeyString::from_redis_module_string(self.ctx, client_username)
    }

    pub fn get_client_cert(&self) -> ValkeyString {
        let client_id = self.get_client_id();
        let client_cert = unsafe { RedisModule_GetClientCertificate.unwrap()(self.ctx, client_id) };
        ValkeyString::from_redis_module_string(self.ctx, client_cert)
    }

    pub fn get_client_info(&self) -> RedisModuleClientInfo {
        let client_id = self.get_client_id();
        let mut mci = RedisModuleClientInfo::new();
        let mci_ptr: *mut c_void = &mut mci as *mut _ as *mut c_void;
        unsafe { RedisModule_GetClientInfoById.unwrap()(mci_ptr, client_id) };
        mci
    }
}
