use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{
    valkey_module, Context, NextArg, ValkeyError, ValkeyResult, ValkeyString, ValkeyValue,
};

fn string_set(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() < 3 {
        return Err(ValkeyError::WrongArity);
    }

    let mut args = args.into_iter().skip(1);
    let key_name = args.next_arg()?;
    let value = args.next_arg()?;

    let key = ctx.open_key_writable(&key_name);
    let mut dma = key.as_string_dma()?;
    dma.write(value.as_slice())
        .map(|_| ValkeyValue::SimpleStringStatic("OK"))
}

fn string_get(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() < 2 {
        return Err(ValkeyError::WrongArity);
    }

    let mut args = args.into_iter().skip(1);
    let key_name = args.next_arg()?;

    let key = ctx.open_key(&key_name);
    let res = key.read()?.map_or(ValkeyValue::Null, |v| {
        ValkeyValue::StringBuffer(Vec::from(v))
    });
    Ok(res)
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "string",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["string.set", string_set, "write fast deny-oom", 1, 1, 1],
        ["string.get", string_get, "readonly", 1, 1, 1],
    ],
}
