use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use valkey_module::alloc::ValkeyAlloc;
use valkey_module::ValkeyError;
use valkey_module::{valkey_module, Context, ValkeyResult, ValkeyString, ValkeyValue};
use valkey_module_macros::{command, ValkeyValue};

#[derive(ValkeyValue)]
struct ValkeyValueDeriveInner {
    i1: i64,
}

#[derive(ValkeyValue)]
struct ValkeyValueDerive {
    i: i64,
    f: f64,
    s: String,
    u: usize,
    v: Vec<i64>,
    #[ValkeyValueAttr{flatten: true}]
    inner: ValkeyValueDeriveInner,
    v2: Vec<ValkeyValueDeriveInner>,
    hash_map: HashMap<String, String>,
    hash_set: HashSet<String>,
    ordered_map: BTreeMap<String, ValkeyValueDeriveInner>,
    ordered_set: BTreeSet<String>,
}

#[derive(ValkeyValue)]
enum ValkeyValueEnum {
    Str(String),
    ValkeyValue(ValkeyValueDerive),
}

#[command(
    {
        flags: [ReadOnly, NoMandatoryKeys],
        arity: -1,
        key_spec: [
            {
                notes: "test valkey value derive macro",
                flags: [ReadOnly, Access],
                begin_search: Index({ index : 0 }),
                find_keys: Range({ last_key : 0, steps : 0, limit : 0 }),
            }
        ]
    }
)]
fn redis_value_derive(
    _ctx: &Context,
    args: Vec<ValkeyString>,
) -> Result<ValkeyValueEnum, ValkeyError> {
    if args.len() > 1 {
        Ok(ValkeyValueEnum::Str("OK".to_owned()))
    } else {
        Ok(ValkeyValueEnum::ValkeyValue(ValkeyValueDerive {
            i: 10,
            f: 1.1,
            s: "s".to_owned(),
            u: 20,
            v: vec![1, 2, 3],
            inner: ValkeyValueDeriveInner { i1: 1 },
            v2: vec![
                ValkeyValueDeriveInner { i1: 1 },
                ValkeyValueDeriveInner { i1: 2 },
            ],
            hash_map: HashMap::from([("key".to_owned(), "val".to_owned())]),
            hash_set: HashSet::from(["key".to_owned()]),
            ordered_map: BTreeMap::from([("key".to_owned(), ValkeyValueDeriveInner { i1: 10 })]),
            ordered_set: BTreeSet::from(["key".to_owned()]),
        }))
    }
}

#[command(
    {
        flags: [ReadOnly],
        arity: -2,
        key_spec: [
            {
                notes: "test command that define all the arguments at even possition as keys",
                flags: [ReadOnly, Access],
                begin_search: Index({ index : 1 }),
                find_keys: Range({ last_key :- 1, steps : 2, limit : 0 }),
            }
        ]
    }
)]
fn classic_keys(_ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ValkeyValue::SimpleStringStatic("OK"))
}

#[command(
    {
        name: "keyword_keys",
        flags: [ReadOnly],
        arity: -2,
        key_spec: [
            {
                notes: "test command that define all the arguments at even possition as keys",
                flags: [ReadOnly, Access],
                begin_search: Keyword({ keyword : "foo", startfrom : 1 }),
                find_keys: Range({ last_key :- 1, steps : 2, limit : 0 }),
            }
        ]
    }
)]
fn keyword_keys(_ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ValkeyValue::SimpleStringStatic("OK"))
}

#[command(
    {
        name: "num_keys",
        flags: [ReadOnly, NoMandatoryKeys],
        arity: -2,
        key_spec: [
            {
                notes: "test command that define all the arguments at even possition as keys",
                flags: [ReadOnly, Access],
                begin_search: Index({ index : 1 }),
                find_keys: Keynum({ key_num_idx : 0, first_key : 1, key_step : 1 }),
            }
        ]
    }
)]
fn num_keys(_ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ValkeyValue::SimpleStringStatic("OK"))
}

valkey_module! {
    name: "server_events",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [],
}
