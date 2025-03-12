use valkey_module::alloc::ValkeyAlloc;
use valkey_module::logging::log_notice;
use valkey_module::{valkey_module, RedisModuleCommandFilterCtx, VALKEYMODULE_CMDFILTER_NOSELF};

// this module shows how to register filters using valkey_module! macro

extern "C" fn my_filter_fn(_ctx: *mut RedisModuleCommandFilterCtx) {
    log_notice("my_filter_fn");
}

extern "C" fn my_filter_fn2(_ctx: *mut RedisModuleCommandFilterCtx) {
    log_notice("my_filter_fn2");
}

valkey_module! {
    name: "filter2",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [],
    filters: [
        [my_filter_fn, VALKEYMODULE_CMDFILTER_NOSELF],
        [my_filter_fn2, VALKEYMODULE_CMDFILTER_NOSELF]
    ],
}
