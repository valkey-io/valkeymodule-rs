use super::CommandFilterCtxTrait;
use crate::{CommandFilterCtx, RedisModuleString};
use std::ffi::c_int;
use std::str::Utf8Error;

impl CommandFilterCtxTrait for CommandFilterCtx {
    fn args_count(&self) -> c_int {
        CommandFilterCtx::args_count(self)
    }

    fn arg_get(&self, pos: c_int) -> *mut RedisModuleString {
        CommandFilterCtx::arg_get(&self, pos)
    }

    fn arg_get_try_as_str<'a>(&self, pos: c_int) -> Result<&'a str, Utf8Error> {
        CommandFilterCtx::arg_get_try_as_str(self, pos)
    }

    fn cmd_get_try_as_str<'a>(&self) -> Result<&'a str, Utf8Error> {
        CommandFilterCtx::cmd_get_try_as_str(self)
    }

    fn get_all_args_wo_cmd<'a>(&self) -> Vec<&'a str> {
        CommandFilterCtx::get_all_args_wo_cmd(self)
    }

    fn arg_replace(&self, pos: c_int, arg: &str) {
        CommandFilterCtx::arg_replace(self, pos, arg);
    }

    fn arg_insert(&self, pos: c_int, arg: &str) {
        CommandFilterCtx::arg_insert(self, pos, arg);
    }

    fn arg_delete(&self, pos: c_int) {
        CommandFilterCtx::arg_delete(self, pos);
    }

    #[cfg(all(any(
        feature = "min-redis-compatibility-version-7-2",
        feature = "min-valkey-compatibility-version-8-0"
    ),))]
    fn get_client_id(&self) -> u64 {
        CommandFilterCtx::get_client_id(self)
    }
}
