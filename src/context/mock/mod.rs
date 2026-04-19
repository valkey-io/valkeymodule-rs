//! Mockable trait abstractions over context wrappers used by the crate.
//!
//! These traits mirror subsets of the concrete context APIs so module logic can
//! be written generically and unit-tested with `mockall`-generated mocks instead
//! of requiring a running Valkey server.

mod cmd_filter_ctx_impl;
mod cmd_filter_ctx_trait;
mod context_impl;
mod context_trait;
mod info_context_impl;
mod info_context_trait;

pub use self::cmd_filter_ctx_trait::CommandFilterCtxTrait;
#[cfg(any(test, feature = "test-mocks"))]
pub use self::cmd_filter_ctx_trait::MockCommandFilterCtxTrait as MockCommandFilterCtx;
pub use self::context_trait::ContextTrait;
#[cfg(any(test, feature = "test-mocks"))]
pub use self::context_trait::MockContextTrait as MockContext;
pub use self::info_context_trait::InfoContextTrait;
#[cfg(any(test, feature = "test-mocks"))]
pub use self::info_context_trait::MockInfoContextTrait as MockInfoContext;
