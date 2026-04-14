use crate::{Context, RedisModuleClientInfo, Status, ValkeyResult, ValkeyString};

/// Mockable interface for the client-related methods on [`Context`] (defined
/// in `src/context/client.rs`). Mirrors `ContextInterface` but isolated so
/// handlers that only touch client state don't depend on the full context
/// surface.
#[cfg_attr(any(test, feature = "test-mocks"), mockall::automock)]
pub trait ContextInterface {
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
}

impl ContextInterface for Context {
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
}

#[cfg(any(test, feature = "test-mocks"))]
pub use self::MockContextInterface as MockContext;

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::eq;

    #[test]
    fn test_client_methods() {
        let mut ctx = MockContext::new();
        ctx.expect_get_client_id().times(1).returning(|| 42);
        ctx.expect_get_client_name_by_id()
            .with(eq(42u64))
            .times(1)
            .returning(|_| Ok(ValkeyString::create_for_test("alice")));
        ctx.expect_set_client_name_by_id()
            .withf(|id, name| *id == 42 && name.as_slice() == b"bob")
            .times(1)
            .returning(|_, _| Status::Ok);
        ctx.expect_get_client_ip()
            .times(1)
            .returning(|| Ok("127.0.0.1".to_string()));
        ctx.expect_deauthenticate_and_close_client()
            .times(1)
            .returning(|| Status::Ok);

        assert_eq!(ctx.get_client_id(), 42);
        assert_eq!(ctx.get_client_name_by_id(42).unwrap().as_slice(), b"alice");
        let bob = ValkeyString::create_for_test("bob");
        assert_eq!(ctx.set_client_name_by_id(42, &bob), Status::Ok);
        assert_eq!(ctx.get_client_ip().unwrap(), "127.0.0.1");
        assert_eq!(ctx.deauthenticate_and_close_client(), Status::Ok);
    }

    #[test]
    fn test_client_info() {
        let mut ctx = MockContext::new();
        ctx.expect_get_client_info_by_id()
            .with(eq(7u64))
            .times(1)
            .returning(|_| {
                Ok(RedisModuleClientInfo {
                    version: 1,
                    flags: 0,
                    id: 7,
                    addr: [0; 46],
                    port: 6379,
                    db: 0,
                })
            });

        let info = ctx.get_client_info_by_id(7).unwrap();
        assert_eq!(info.id, 7);
        assert_eq!(info.port, 6379);
    }
}
