use std::time::Duration;
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{valkey_module, Context, NextArg, ValkeyError, ValkeyResult, ValkeyString};

fn expire_cmd(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() < 3 {
        return Err(ValkeyError::WrongArity);
    }

    let mut args = args.into_iter().skip(1);
    let key_name = args.next_arg()?;
    let ttl_sec = args.next_i64()?;
    let key = ctx.open_key_writable(&key_name);
    if ttl_sec >= 0 {
        key.set_expire(Duration::new(ttl_sec as u64, 0))
    } else {
        key.remove_expire()
    }
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "expire",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["expire.cmd", expire_cmd, "write fast deny-oom", 1, 1, 1],
    ],
}
