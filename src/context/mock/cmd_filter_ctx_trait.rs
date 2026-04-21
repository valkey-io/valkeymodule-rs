use crate::RedisModuleString;
use std::ffi::c_int;
use std::str::Utf8Error;

#[cfg_attr(any(test, feature = "test-mocks"), mockall::automock)]
pub trait CommandFilterCtxTrait {
    fn args_count(&self) -> c_int;
    fn arg_get(&self, pos: c_int) -> *mut RedisModuleString;
    fn arg_get_try_as_str<'a>(&self, pos: c_int) -> Result<&'a str, Utf8Error>;
    fn cmd_get_try_as_str<'a>(&self) -> Result<&'a str, Utf8Error>;
    fn get_all_args_wo_cmd<'a>(&self) -> Vec<&'a str>;

    fn arg_replace(&self, pos: c_int, arg: &str);
    fn arg_insert(&self, pos: c_int, arg: &str);
    fn arg_delete(&self, pos: c_int);

    #[cfg(all(any(
        feature = "min-redis-compatibility-version-7-2",
        feature = "min-valkey-compatibility-version-8-0"
    ),))]
    fn get_client_id(&self) -> u64;
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::eq;

    #[test]
    fn test_dispatches_through_impl_and_dyn() {
        fn static_dispatch(ctx: &impl CommandFilterCtxTrait) {
            ctx.arg_replace(0, "info2");
        }

        fn dynamic_dispatch(ctx: &dyn CommandFilterCtxTrait) {
            ctx.arg_replace(0, "info2");
        }

        let mut ctx = MockCommandFilterCtxTrait::new();
        ctx.expect_arg_replace()
            .with(eq(0), eq("info2"))
            .times(2)
            .return_const(());

        static_dispatch(&ctx);
        dynamic_dispatch(&ctx);
    }
}
