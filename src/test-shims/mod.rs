mod call;
mod command_filter_ctx;
mod context;
mod info_context;
mod thread_safe;
mod valkey_string;

pub use command_filter_ctx::TestCommandFilterCtx;
pub(crate) use context::try_call;
pub use context::TestContext;
pub use info_context::{
    TestInfoContext, TestInfoEntry, TestInfoField, TestInfoSection, TestInfoValue,
};
pub use thread_safe::TestThreadSafeContext;

use crate::raw;
use std::ptr::addr_of;
use std::sync::Once;

static INIT: Once = Once::new();

/// Creates owned, binary-safe test command arguments from byte-like values.
pub fn create_test_args<T: AsRef<[u8]>>(args: &[T]) -> Vec<crate::ValkeyString> {
    args.iter()
        .map(|arg| crate::ValkeyString::test(arg.as_ref()))
        .collect()
}

fn setup_test_shims() {
    INIT.call_once(|| {
        assert!(
            !real_valkey_api_is_initialized(),
            "refusing to install test shims inside a running Valkey process"
        );

        valkey_string::install();
        context::install();
        call::install();
        info_context::install();
        command_filter_ctx::install();
        thread_safe::install();
    });
}

fn real_valkey_api_is_initialized() -> bool {
    // SAFETY: These function pointers are initialized by the real module API setup.
    // Reading copies through raw pointers avoids borrowing the mutable statics.
    unsafe {
        addr_of!(raw::RedisModule_GetApi).read().is_some()
            || addr_of!(raw::ValkeyModule_GetApi).read().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_binary_safe_test_arguments() {
        let input = [b"text".as_slice(), b"\0\xff".as_slice()];

        let args = create_test_args(&input);

        assert_eq!(args[0].as_slice(), b"text");
        assert_eq!(args[1].as_slice(), b"\0\xff");
    }
}
