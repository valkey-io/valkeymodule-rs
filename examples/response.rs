use std::collections::{BTreeMap, BTreeSet};
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{
    redisvalue::ValkeyValueKey, valkey_module, Context, NextArg, ValkeyError, ValkeyResult,
    ValkeyString, ValkeyValue,
};

fn map_mget(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() < 2 {
        return Err(ValkeyError::WrongArity);
    }

    let mut args = args.into_iter().skip(1);
    let key_name = args.next_arg()?;

    let fields: Vec<ValkeyString> = args.collect();

    let key = ctx.open_key(&key_name);
    let values = key.hash_get_multi(&fields)?;
    let res = match values {
        None => ValkeyValue::Null,
        Some(values) => {
            let mut map: BTreeMap<ValkeyValueKey, ValkeyValue> = BTreeMap::new();
            for (field, value) in values.into_iter() {
                map.insert(
                    ValkeyValueKey::BulkValkeyString(field),
                    ValkeyValue::BulkValkeyString(value),
                );
            }
            ValkeyValue::OrderedMap(map)
        }
    };

    Ok(res)
}

fn map_unique(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() < 2 {
        return Err(ValkeyError::WrongArity);
    }

    let mut args = args.into_iter().skip(1);
    let key_name = args.next_arg()?;

    let fields: Vec<ValkeyString> = args.collect();

    let key = ctx.open_key(&key_name);
    let values = key.hash_get_multi(&fields)?;
    let res = match values {
        None => ValkeyValue::Null,
        Some(values) => {
            let mut set: BTreeSet<ValkeyValueKey> = BTreeSet::new();
            for (_, value) in values.into_iter() {
                set.insert(ValkeyValueKey::BulkValkeyString(value));
            }
            ValkeyValue::OrderedSet(set)
        }
    };

    Ok(res)
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "response",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["map.mget", map_mget, "readonly", 1, 1, 1],
        ["map.unique", map_unique, "readonly", 1, 1, 1],
    ],
}
