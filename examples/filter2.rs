use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{valkey_module, RedisModuleCommandFilterCtx, VALKEYMODULE_CMDFILTER_NOSELF};

fn filter1_fn(_ctx: *mut RedisModuleCommandFilterCtx) {
    // do something here, registered via valkey_module! macro
    // making sure that two modules can have the same filter fn name
}

fn filter2_fn(_ctx: *mut RedisModuleCommandFilterCtx) {
    // do something here, registered via valkey_module! macro
    // making sure that two modules can have the same filter fn name
}

valkey_module! {
    name: "filter2",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
    ],
    filters: [
        [filter1_fn, VALKEYMODULE_CMDFILTER_NOSELF],
        [filter2_fn, VALKEYMODULE_CMDFILTER_NOSELF]
    ]
}
