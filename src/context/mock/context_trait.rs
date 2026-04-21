use crate::logging::ValkeyLogLevel;
use crate::{RedisModuleClientInfo, Status, ValkeyResult, ValkeyString};

#[cfg_attr(any(test, feature = "test-mocks"), mockall::automock)]
pub trait ContextTrait {
    fn log(&self, level: ValkeyLogLevel, message: &str);
    fn log_debug(&self, message: &str) {
        self.log(ValkeyLogLevel::Debug, message);
    }
    fn log_notice(&self, message: &str) {
        self.log(ValkeyLogLevel::Notice, message);
    }
    fn log_verbose(&self, message: &str) {
        self.log(ValkeyLogLevel::Verbose, message);
    }
    fn log_warning(&self, message: &str) {
        self.log(ValkeyLogLevel::Warning, message);
    }
    fn create_string(&self, s: &str) -> ValkeyString;
    fn get_current_user(&self) -> ValkeyString;
    fn call<'a>(&self, command: &str, args: &'a [&'a str]) -> ValkeyResult;
    fn set_module_options(&self, options: crate::raw::ModuleOptions);
    fn get_server_version(&self) -> ValkeyResult<crate::raw::Version>;

    // auth methods
    fn authenticate_client_with_acl_user(&self, username: &ValkeyString) -> Status;

    // client methods
    fn get_client_id(&self) -> u64;
    fn get_client_name_by_id(&self, client_id: u64) -> ValkeyResult<ValkeyString>;
    fn get_client_name(&self) -> ValkeyResult<ValkeyString>;
    fn set_client_name_by_id(&self, client_id: u64, client_name: &ValkeyString) -> Status;
    fn set_client_name(&self, client_name: &ValkeyString) -> Status;
    fn get_client_username_by_id(&self, client_id: u64) -> ValkeyResult<ValkeyString>;
    fn get_client_username(&self) -> ValkeyResult<ValkeyString>;
    fn get_client_cert(&self) -> ValkeyResult<ValkeyString>;
    fn get_client_info_by_id(&self, client_id: u64) -> ValkeyResult<RedisModuleClientInfo>;
    fn get_client_info(&self) -> ValkeyResult<RedisModuleClientInfo>;
    fn get_client_ip_by_id(&self, client_id: u64) -> ValkeyResult<String>;
    fn get_client_ip(&self) -> ValkeyResult<String>;
    fn deauthenticate_and_close_client_by_id(&self, client_id: u64) -> Status;
    fn deauthenticate_and_close_client(&self) -> Status;
    fn config_get(&self, config: String) -> ValkeyResult<ValkeyString>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ValkeyValue;
    use mockall::predicate::eq;

    #[test]
    fn test_dispatches_through_impl_and_dyn() {
        fn static_dispatch(ctx: &impl ContextTrait) {
            ctx.log_notice("hi");
        }
        fn dynamic_dispatch(ctx: &dyn ContextTrait) {
            ctx.log_notice("hi");
        }

        let mut ctx = MockContextTrait::new();
        ctx.expect_log_notice()
            .with(eq("hi"))
            .times(2)
            .return_const(());
        static_dispatch(&ctx);
        dynamic_dispatch(&ctx);
    }

    #[test]
    fn test_call() {
        let mut ctx = MockContextTrait::new();
        ctx.expect_call()
            .withf(|cmd, args| cmd == "SET" && args == ["key", "val"])
            .times(1)
            .returning(|_, _| Ok(ValkeyValue::SimpleStringStatic("OK")));

        let res = ctx.call("SET", &["key", "val"]).unwrap();
        assert!(matches!(res, ValkeyValue::SimpleStringStatic("OK")));
    }
}
