use valkey_module::valkey_module;

valkey_module! {
    name: "empty_config_group",
    version: 1,
    allocator: (std::alloc::System, std::alloc::System),
    data_types: [],
    commands: [],
    configurations: [
        group: [],
    ]
}

fn main() {}
