use valkey_module::{
    key::KeyFlags, valkey_module, Context, NextArg, ValkeyError, ValkeyResult, ValkeyString,
    ValkeyValue,
};
use valkey_module_macros::command;

#[command(
    {
        name: "open_key_with_flags.read",
        flags: [Write, DenyOOM],
        arity: 2,
        key_spec: [
            {
                flags: [ReadOnly, Access],
                begin_search: Index({ index : 1 }),
                find_keys: Range({ last_key : 1, steps : 1, limit : 1}),
            }
        ]

    }
)]
fn read(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() < 2 {
        return Err(ValkeyError::WrongArity);
    }

    let mut args = args.into_iter().skip(1);
    let key_name = args.next_arg()?;
    let _ = ctx.open_key_with_flags(&key_name, KeyFlags::NOEFFECTS);
    Ok(ValkeyValue::SimpleStringStatic("OK"))
}

#[command(
    {
        name: "open_key_with_flags.write",
        flags: [Write, DenyOOM],
        arity: 2,
        key_spec: [
            {
                flags: [ReadWrite, Access],
                begin_search: Index({ index : 1 }),
                find_keys: Range({ last_key : 1, steps : 1, limit : 1}),
            }
        ]

    }
)]
fn write(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() < 2 {
        return Err(ValkeyError::WrongArity);
    }

    let mut args = args.into_iter().skip(1);
    let key_name = args.next_arg()?;
    let _ = ctx.open_key_writable_with_flags(&key_name, KeyFlags::NOEFFECTS);
    Ok(ValkeyValue::SimpleStringStatic("OK"))
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "open_key_with_flags",
    version: 1,
    allocator: (valkey_module::alloc::ValkeyAlloc, valkey_module::alloc::ValkeyAlloc),
    data_types: [],
    commands: [],
}
