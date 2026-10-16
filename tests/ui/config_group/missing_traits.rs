use valkey_module::__private::serde::Deserialize;
use valkey_module::config_group;

#[derive(Clone, Deserialize)]
struct MissingSerialize;

config_group! {
    pub missing_traits {
        value: MissingSerialize = MissingSerialize,
    }
}

fn main() {}
