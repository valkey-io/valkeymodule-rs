use valkey_module::config_group;

config_group! {
    pub invalid_range {
        count: i64 = 10, range = 0..100,
    }
}

fn main() {}
