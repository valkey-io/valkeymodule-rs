use valkey_module::config_group;

config_group! {
    pub mismatched_default {
        count: i64 = "ten",
    }
}

fn main() {}
