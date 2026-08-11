use crate::{raw, CommandFilterCtx, ValkeyString};
use std::collections::HashMap;
use std::ops::Deref;
use std::ptr::null_mut;

pub(super) fn install() {
    // SAFETY: `setup_test_shims` calls this once after verifying the real API is uninitialized.
    unsafe {
        raw::RedisModule_CommandFilterArgsCount = Some(command_filter_args_count);
        raw::RedisModule_CommandFilterArgGet = Some(command_filter_arg_get);
        raw::RedisModule_CommandFilterArgReplace = Some(command_filter_arg_replace);
        raw::RedisModule_CommandFilterArgInsert = Some(command_filter_arg_insert);
        raw::RedisModule_CommandFilterArgDelete = Some(command_filter_arg_delete);
        raw::RedisModule_CommandFilterGetClientId = Some(command_filter_get_client_id);
    }
}

const DEFAULT_CLIENT_ID: u64 = 1;

impl CommandFilterCtx {
    /// Creates a test command-filter context whose API return values can be configured.
    #[must_use]
    pub fn test() -> TestCommandFilterCtx {
        TestCommandFilterCtx::new()
    }
}

struct CommandFilterData {
    args_count: libc::c_int,
    args: HashMap<libc::c_int, ValkeyString>,
    client_id: u64,
}

impl Default for CommandFilterData {
    fn default() -> Self {
        Self {
            args_count: 0,
            args: HashMap::new(),
            client_id: DEFAULT_CLIENT_ID,
        }
    }
}

/// Owns a test-only [`CommandFilterCtx`] that can be used without a running Valkey server.
pub struct TestCommandFilterCtx {
    context: CommandFilterCtx,
    data: Box<CommandFilterData>,
}

impl TestCommandFilterCtx {
    fn new() -> Self {
        super::setup_test_shims();

        let mut data = Box::<CommandFilterData>::default();
        let inner =
            (data.as_mut() as *mut CommandFilterData).cast::<raw::RedisModuleCommandFilterCtx>();

        Self {
            context: CommandFilterCtx::new(inner),
            data,
        }
    }

    /// Returns the opaque context pointer for invoking a command-filter callback in a test.
    ///
    /// The pointer remains valid while this test context is alive.
    pub fn as_raw_ctx_ptr(&mut self) -> *mut raw::RedisModuleCommandFilterCtx {
        (self.data.as_mut() as *mut CommandFilterData).cast()
    }

    /// Configures the value returned by [`CommandFilterCtx::args_count`].
    pub fn expect_args_count(&mut self, args_count: libc::c_int) -> &mut Self {
        self.data.args_count = args_count;
        self
    }

    /// Configures the value returned by [`CommandFilterCtx::arg_get`] for `position`.
    pub fn expect_arg_get<T: Into<Vec<u8>>>(
        &mut self,
        position: libc::c_int,
        argument: T,
    ) -> &mut Self {
        self.data
            .args
            .insert(position, ValkeyString::test(argument));
        self
    }

    /// Configures the value returned by [`CommandFilterCtx::get_client_id`].
    pub fn expect_get_client_id(&mut self, client_id: u64) -> &mut Self {
        self.data.client_id = client_id;
        self
    }
}

impl Deref for TestCommandFilterCtx {
    type Target = CommandFilterCtx;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

pub(super) extern "C" fn command_filter_args_count(
    ctx: *mut raw::RedisModuleCommandFilterCtx,
) -> libc::c_int {
    if ctx.is_null() {
        return 0;
    }

    // SAFETY: `TestCommandFilterCtx::new` stores a live `CommandFilterData` allocation at this
    // opaque pointer.
    unsafe { &*ctx.cast::<CommandFilterData>() }.args_count
}

pub(super) extern "C" fn command_filter_arg_get(
    ctx: *mut raw::RedisModuleCommandFilterCtx,
    position: libc::c_int,
) -> *mut raw::RedisModuleString {
    if ctx.is_null() || position < 0 {
        return null_mut();
    }

    // SAFETY: `TestCommandFilterCtx::new` stores a live `CommandFilterData` allocation at this
    // opaque pointer.
    let data = unsafe { &*ctx.cast::<CommandFilterData>() };
    data.args
        .get(&position)
        .map_or(null_mut(), |argument| argument.inner)
}

pub(super) extern "C" fn command_filter_arg_replace(
    ctx: *mut raw::RedisModuleCommandFilterCtx,
    position: libc::c_int,
    argument: *mut raw::RedisModuleString,
) -> libc::c_int {
    if argument.is_null() {
        return raw::Status::Err as libc::c_int;
    }

    // `CommandFilterCtx::arg_replace` transfers one retained reference to the callback. Taking
    // ownership here ensures that reference is released even when the replacement is rejected.
    let argument = ValkeyString::from_redis_module_string(null_mut(), argument);
    if ctx.is_null() || position < 0 {
        return raw::Status::Err as libc::c_int;
    }

    // SAFETY: `TestCommandFilterCtx::new` stores a live, uniquely owned `CommandFilterData`
    // allocation at this opaque pointer. Command-filter callbacks execute synchronously.
    let data = unsafe { &mut *ctx.cast::<CommandFilterData>() };
    let Some(current_argument) = data.args.get_mut(&position) else {
        return raw::Status::Err as libc::c_int;
    };

    *current_argument = argument;
    raw::Status::Ok as libc::c_int
}

pub(super) extern "C" fn command_filter_arg_insert(
    ctx: *mut raw::RedisModuleCommandFilterCtx,
    position: libc::c_int,
    argument: *mut raw::RedisModuleString,
) -> libc::c_int {
    if argument.is_null() {
        return raw::Status::Err as libc::c_int;
    }

    // `CommandFilterCtx::arg_insert` transfers one retained reference to the callback. Taking
    // ownership here ensures that reference is released even when the insertion is rejected.
    let argument = ValkeyString::from_redis_module_string(null_mut(), argument);
    if ctx.is_null() || position < 0 {
        return raw::Status::Err as libc::c_int;
    }

    // SAFETY: `TestCommandFilterCtx::new` stores a live, uniquely owned `CommandFilterData`
    // allocation at this opaque pointer. Command-filter callbacks execute synchronously.
    let data = unsafe { &mut *ctx.cast::<CommandFilterData>() };
    if data.args_count < 0 || position > data.args_count || data.args_count == libc::c_int::MAX {
        return raw::Status::Err as libc::c_int;
    }

    for current_position in (position..data.args_count).rev() {
        if let Some(current_argument) = data.args.remove(&current_position) {
            data.args.insert(current_position + 1, current_argument);
        }
    }
    data.args.insert(position, argument);
    data.args_count += 1;

    raw::Status::Ok as libc::c_int
}

pub(super) extern "C" fn command_filter_arg_delete(
    ctx: *mut raw::RedisModuleCommandFilterCtx,
    position: libc::c_int,
) -> libc::c_int {
    if ctx.is_null() || position < 0 {
        return raw::Status::Err as libc::c_int;
    }

    // SAFETY: `TestCommandFilterCtx::new` stores a live, uniquely owned `CommandFilterData`
    // allocation at this opaque pointer. Command-filter callbacks execute synchronously.
    let data = unsafe { &mut *ctx.cast::<CommandFilterData>() };
    if position >= data.args_count {
        return raw::Status::Err as libc::c_int;
    }

    data.args.remove(&position);
    for current_position in position + 1..data.args_count {
        if let Some(current_argument) = data.args.remove(&current_position) {
            data.args.insert(current_position - 1, current_argument);
        }
    }
    data.args_count -= 1;

    raw::Status::Ok as libc::c_int
}

pub(super) extern "C" fn command_filter_get_client_id(
    ctx: *mut raw::RedisModuleCommandFilterCtx,
) -> libc::c_ulonglong {
    if ctx.is_null() {
        return DEFAULT_CLIENT_ID;
    }

    // SAFETY: `TestCommandFilterCtx::new` stores a live `CommandFilterData` allocation at this
    // opaque pointer.
    unsafe { &*ctx.cast::<CommandFilterData>() }.client_id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_configured_args_count() {
        let mut context = CommandFilterCtx::test();
        context.expect_args_count(3);

        assert_eq!(context.args_count(), 3);
    }

    #[test]
    fn defaults_args_count_to_zero() {
        let context = CommandFilterCtx::test();

        assert_eq!(context.args_count(), 0);
    }

    #[test]
    fn replaces_configured_args_count() {
        let mut context = CommandFilterCtx::test();
        context.expect_args_count(1).expect_args_count(4);

        assert_eq!(context.args_count(), 4);
    }

    #[test]
    fn returns_zero_for_null_context() {
        assert_eq!(command_filter_args_count(null_mut()), 0);
    }

    #[test]
    fn returns_configured_argument() {
        let mut context = CommandFilterCtx::test();
        context.expect_arg_get(1, "value");

        assert_eq!(
            context
                .arg_get_try_as_str(1)
                .expect("configured argument should be valid UTF-8"),
            "value"
        );
    }

    #[test]
    fn returns_configured_command() {
        let mut context = CommandFilterCtx::test();
        context.expect_arg_get(0, "set");

        assert_eq!(
            context
                .cmd_get_try_as_str()
                .expect("configured command should be valid UTF-8"),
            "set"
        );
    }

    #[test]
    fn replaces_configured_argument() {
        let mut context = CommandFilterCtx::test();
        context.expect_arg_get(1, "old").expect_arg_get(1, "new");

        assert_eq!(
            context
                .arg_get_try_as_str(1)
                .expect("replacement argument should be valid UTF-8"),
            "new"
        );
    }

    #[test]
    fn returns_null_for_unconfigured_or_negative_position() {
        let context = CommandFilterCtx::test();

        assert!(context.arg_get(0).is_null());
        assert!(context.arg_get(-1).is_null());
    }

    #[test]
    fn returns_null_argument_for_null_context() {
        assert!(command_filter_arg_get(null_mut(), 0).is_null());
    }

    #[test]
    fn replaces_argument() {
        let mut context = CommandFilterCtx::test();
        context.expect_arg_get(1, "old");

        context.arg_replace(1, "new");

        assert_eq!(
            context
                .arg_get_try_as_str(1)
                .expect("replacement argument should be valid UTF-8"),
            "new"
        );
    }

    #[test]
    fn rejects_null_argument_or_context() {
        assert_eq!(
            command_filter_arg_replace(null_mut(), 0, null_mut()),
            raw::Status::Err as libc::c_int
        );

        let argument = ValkeyString::create_and_retain("new");
        assert_eq!(
            command_filter_arg_replace(null_mut(), 0, argument.inner),
            raw::Status::Err as libc::c_int
        );
    }

    #[test]
    fn inserts_argument_and_shifts_later_arguments() {
        let mut context = CommandFilterCtx::test();
        context
            .expect_args_count(3)
            .expect_arg_get(0, "set")
            .expect_arg_get(1, "key")
            .expect_arg_get(2, "value");

        context.arg_insert(1, "new-key");

        assert_eq!(context.args_count(), 4);
        assert_eq!(context.cmd_get_try_as_str(), Ok("set"));
        assert_eq!(context.arg_get_try_as_str(1), Ok("new-key"));
        assert_eq!(context.arg_get_try_as_str(2), Ok("key"));
        assert_eq!(context.arg_get_try_as_str(3), Ok("value"));
    }

    #[test]
    fn appends_argument_at_args_count() {
        let mut context = CommandFilterCtx::test();
        context.expect_args_count(1).expect_arg_get(0, "command");

        context.arg_insert(1, "argument");

        assert_eq!(context.args_count(), 2);
        assert_eq!(context.arg_get_try_as_str(1), Ok("argument"));
    }

    #[test]
    fn rejects_insert_outside_argument_bounds() {
        let mut context = CommandFilterCtx::test();
        context.expect_args_count(1).expect_arg_get(0, "command");

        context.arg_insert(2, "argument");
        context.arg_insert(-1, "argument");

        assert_eq!(context.args_count(), 1);
        assert!(context.arg_get(1).is_null());
    }

    #[test]
    fn rejects_insert_with_null_argument_or_context() {
        assert_eq!(
            command_filter_arg_insert(null_mut(), 0, null_mut()),
            raw::Status::Err as libc::c_int
        );

        let argument = ValkeyString::create_and_retain("new");
        assert_eq!(
            command_filter_arg_insert(null_mut(), 0, argument.inner),
            raw::Status::Err as libc::c_int
        );
    }

    #[test]
    fn deletes_argument_and_shifts_later_arguments() {
        let mut context = CommandFilterCtx::test();
        context
            .expect_args_count(4)
            .expect_arg_get(0, "set")
            .expect_arg_get(1, "old-key")
            .expect_arg_get(2, "new-key")
            .expect_arg_get(3, "value");

        context.arg_delete(1);

        assert_eq!(context.args_count(), 3);
        assert_eq!(context.cmd_get_try_as_str(), Ok("set"));
        assert_eq!(context.arg_get_try_as_str(1), Ok("new-key"));
        assert_eq!(context.arg_get_try_as_str(2), Ok("value"));
        assert!(context.arg_get(3).is_null());
    }

    #[test]
    fn deletes_command_and_shifts_arguments() {
        let mut context = CommandFilterCtx::test();
        context
            .expect_args_count(2)
            .expect_arg_get(0, "command")
            .expect_arg_get(1, "argument");

        context.arg_delete(0);

        assert_eq!(context.args_count(), 1);
        assert_eq!(context.cmd_get_try_as_str(), Ok("argument"));
    }

    #[test]
    fn rejects_delete_outside_argument_bounds() {
        let mut context = CommandFilterCtx::test();
        context.expect_args_count(1).expect_arg_get(0, "command");

        context.arg_delete(1);
        context.arg_delete(-1);

        assert_eq!(context.args_count(), 1);
        assert_eq!(context.cmd_get_try_as_str(), Ok("command"));
    }

    #[test]
    fn rejects_delete_with_null_context() {
        assert_eq!(
            command_filter_arg_delete(null_mut(), 0),
            raw::Status::Err as libc::c_int
        );
    }

    #[test]
    fn returns_configured_client_id() {
        let mut context = CommandFilterCtx::test();
        context.expect_get_client_id(42);

        assert_eq!(context.get_client_id(), 42);
    }

    #[test]
    fn defaults_client_id_when_no_expectation_is_configured() {
        let context = CommandFilterCtx::test();

        assert_eq!(context.get_client_id(), DEFAULT_CLIENT_ID);
    }

    #[test]
    fn replaces_configured_client_id() {
        let mut context = CommandFilterCtx::test();
        context.expect_get_client_id(10).expect_get_client_id(20);

        assert_eq!(context.get_client_id(), 20);
    }

    #[test]
    fn exposes_raw_context_pointer_for_command_filter_callbacks() {
        extern "C" fn replace_argument(ctx: *mut raw::RedisModuleCommandFilterCtx) {
            CommandFilterCtx::new(ctx).arg_replace(1, "new");
        }

        let mut context = CommandFilterCtx::test();
        context.expect_args_count(2).expect_arg_get(1, "old");

        replace_argument(context.as_raw_ctx_ptr());

        assert_eq!(context.arg_get_try_as_str(1), Ok("new"));
    }

    #[test]
    fn defaults_client_id_for_null_context() {
        assert_eq!(command_filter_get_client_id(null_mut()), DEFAULT_CLIENT_ID);
    }
}
