use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{valkey_module, ContextTrait, Status, ValkeyString};

fn preload(ctx: &impl ContextTrait, args: &[ValkeyString]) -> Status {
    // perform preload validations here, useful for MODULE LOAD
    // unlike init which is called at the end of the valkey_module! macro this is called at the beginning
    let version = ctx.get_server_version().unwrap();
    ctx.log_notice(&format!(
        "preload for server version {:?} with args: {:?}",
        version, args
    ));
    // respond with either Status::Ok or Status::Err (if you want to prevent module loading)
    Status::Ok
}

valkey_module! {
    name: "preload",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    preload: preload,
    commands: [],
}

#[cfg(test)]
mod tests {
    use super::*;
    use valkey_module::raw::Version;
    use valkey_module::MockContext;

    #[test]
    fn test_preload_calls_get_server_version() {
        let mut ctx = MockContext::new();
        ctx.expect_get_server_version().times(1).returning(|| {
            Ok(Version {
                major: 8,
                minor: 0,
                patch: 1,
            })
        });
        ctx.expect_log_notice().times(1).return_const(());

        let status = preload(&ctx, &[]);
        assert!(matches!(status, Status::Ok));
    }
}
