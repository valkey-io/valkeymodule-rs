/// Mockable trait abstractions over [`Context`].
///
/// This trait mirrors the `Context` methods so that module logic can be
/// unit-tested without a running Valkey server.
///
/// Production code can accept `&impl ContextInterface` or `&dyn ContextInterface`
/// instead of a concrete [`Context`] to keep command logic unit-testable.
///
/// The corresponding `MockContext` type is only exported when compiling tests or
/// when the crate is built with the `test-mocks` feature enabled.
///
/// This module mirrors the layout of [`crate::context`]: each submodule here
/// corresponds to the like-named file under `src/context/`, and exposes a
/// trait (plus a `mockall`-generated mock) covering that file's methods on
/// `Context`. For example, `mock::client` mirrors `context::client`.
///
/// The trait in this file, [`ContextInterface`], covers the core `Context`
/// methods (logging, string creation, `CONFIG GET`). Submodule traits scope
/// their own method sets so handlers can depend on only the surface they
/// actually use.

///
/// # Examples
///
/// ```
/// # #[cfg(feature = "test-mocks")]
/// # {
/// use valkey_module::{ContextInterface, MockContext};
///
/// fn emit_notice(ctx: &impl ContextInterface) {
///     ctx.log_notice("ready");
/// }
///
/// let mut ctx = MockContext::new();
/// ctx.expect_log_notice()
///     .withf(|msg| msg == "ready")
///     .times(1)
///     .return_const(());
///
/// emit_notice(&ctx);
/// # }
/// ```
pub mod client;

use crate::logging::ValkeyLogLevel;
use crate::{Context, ValkeyResult, ValkeyString};

#[cfg_attr(any(test, feature = "test-mocks"), mockall::automock)]
pub trait ContextInterface {
    fn log(&self, _level: ValkeyLogLevel, _message: &str) {}
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
    fn create_string(&self, s: &[u8]) -> ValkeyString;
    fn config_get(&self, config: &str) -> ValkeyResult<ValkeyString>;
}

impl ContextInterface for Context {
    fn log(&self, level: ValkeyLogLevel, message: &str) {
        Context::log(self, level, message);
    }

    fn create_string(&self, s: &[u8]) -> ValkeyString {
        Context::create_string(self, s)
    }

    fn config_get(&self, config: &str) -> ValkeyResult<ValkeyString> {
        Context::config_get(self, config.into())
    }
}

#[cfg(any(test, feature = "test-mocks"))]
pub use self::MockContextInterface as MockContext;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ValkeyError;
    use mockall::predicate::eq;
    use std::str::from_utf8;

    #[test]
    fn test_log_methods() {
        let mut ctx = MockContext::new();
        ctx.expect_log()
            .withf(|lvl, msg| {
                matches!(lvl, ValkeyLogLevel::Warning) && msg == "something went wrong"
            })
            .times(1)
            .return_const(());
        ctx.expect_log_debug()
            .with(eq("d"))
            .times(1)
            .return_const(());
        ctx.expect_log_notice()
            .with(eq("n"))
            .times(1)
            .return_const(());
        ctx.expect_log_verbose()
            .with(eq("v"))
            .times(1)
            .return_const(());
        ctx.expect_log_warning()
            .with(eq("w"))
            .times(1)
            .return_const(());
        ctx.log(ValkeyLogLevel::Warning, "something went wrong");
        ctx.log_debug("d");
        ctx.log_notice("n");
        ctx.log_verbose("v");
        ctx.log_warning("w");
    }

    #[test]
    fn test_create_string() {
        let mut ctx = MockContext::new();
        ctx.expect_create_string()
            .with(eq(b"hello".as_slice()))
            .times(1)
            .returning(|tmp| ValkeyString::create_for_test(from_utf8(tmp).unwrap()));
        let reply = ctx.create_string(b"hello");
        assert_eq!(reply.as_slice(), b"hello");
    }

    #[test]
    fn test_config_get() {
        let mut ctx = MockContext::new();
        ctx.expect_config_get()
            .with(eq("maxmemory"))
            .times(1)
            .returning(|_| Ok(ValkeyString::create_for_test("1073741824")));
        ctx.expect_config_get()
            .with(eq("nonexistent"))
            .times(1)
            .returning(|_| Err(ValkeyError::Str("Unexpected CONFIG GET response")));

        let ok = ctx.config_get("maxmemory").unwrap();
        assert_eq!(ok.as_slice(), b"1073741824");

        let err = ctx.config_get("nonexistent").unwrap_err();
        assert!(matches!(
            err,
            ValkeyError::Str("Unexpected CONFIG GET response")
        ));
    }

    /// Both `&impl ContextInterface` and `&dyn ContextInterface` accept the mock.
    #[test]
    fn test_mock_works_through_generic_and_dyn() {
        fn static_dispatch(ctx: &impl ContextInterface) {
            ctx.log_notice("hi");
        }
        fn dynamic_dispatch(ctx: &dyn ContextInterface) {
            ctx.log_notice("hi");
        }

        let mut ctx = MockContext::new();
        ctx.expect_log_notice()
            .with(eq("hi"))
            .times(2)
            .return_const(());
        static_dispatch(&ctx);
        dynamic_dispatch(&ctx);
    }
}
