//! `ContextTrait` delegations for the real [`Context`], split out so
//! `mod.rs` only shows the mockable surface.

use super::ContextTrait;
use crate::logging::ValkeyLogLevel;
use crate::{Context, RedisModuleClientInfo, Status, ValkeyResult, ValkeyString};

impl ContextTrait for Context {
    fn log(&self, level: ValkeyLogLevel, message: &str) {
        Context::log(self, level, message);
    }
    fn create_string(&self, s: &str) -> ValkeyString {
        Context::create_string(self, s)
    }
    fn get_current_user(&self) -> ValkeyString {
        Context::get_current_user(self)
    }
    // call methods
    fn call<'a>(&self, command: &str, args: &'a [&'a str]) -> ValkeyResult {
        Context::call(self, command, args)
    }
    fn set_module_options(&self, options: crate::raw::ModuleOptions) {
        Context::set_module_options(self, options);
    }
    fn get_server_version(&self) -> ValkeyResult<crate::raw::Version> {
        Context::get_server_version(self)
    }

    // auth methods
    fn authenticate_client_with_acl_user(&self, username: &ValkeyString) -> Status {
        Context::authenticate_client_with_acl_user(self, username)
    }

    // client methods
    fn get_client_id(&self) -> u64 {
        Context::get_client_id(self)
    }
    fn get_client_name_by_id(&self, client_id: u64) -> ValkeyResult<ValkeyString> {
        Context::get_client_name_by_id(self, client_id)
    }
    fn get_client_name(&self) -> ValkeyResult<ValkeyString> {
        Context::get_client_name(self)
    }
    fn set_client_name_by_id(&self, client_id: u64, client_name: &ValkeyString) -> Status {
        Context::set_client_name_by_id(self, client_id, client_name)
    }
    fn set_client_name(&self, client_name: &ValkeyString) -> Status {
        Context::set_client_name(self, client_name)
    }
    fn get_client_username_by_id(&self, client_id: u64) -> ValkeyResult<ValkeyString> {
        Context::get_client_username_by_id(self, client_id)
    }
    fn get_client_username(&self) -> ValkeyResult<ValkeyString> {
        Context::get_client_username(self)
    }
    fn get_client_cert(&self) -> ValkeyResult<ValkeyString> {
        Context::get_client_cert(self)
    }
    fn get_client_info_by_id(&self, client_id: u64) -> ValkeyResult<RedisModuleClientInfo> {
        Context::get_client_info_by_id(self, client_id)
    }
    fn get_client_info(&self) -> ValkeyResult<RedisModuleClientInfo> {
        Context::get_client_info(self)
    }
    fn get_client_ip_by_id(&self, client_id: u64) -> ValkeyResult<String> {
        Context::get_client_ip_by_id(self, client_id)
    }
    fn get_client_ip(&self) -> ValkeyResult<String> {
        Context::get_client_ip(self)
    }
    fn deauthenticate_and_close_client_by_id(&self, client_id: u64) -> Status {
        Context::deauthenticate_and_close_client_by_id(self, client_id)
    }
    fn deauthenticate_and_close_client(&self) -> Status {
        Context::deauthenticate_and_close_client(self)
    }
    fn config_get(&self, config: String) -> ValkeyResult<ValkeyString> {
        Context::config_get(self, config)
    }
}
