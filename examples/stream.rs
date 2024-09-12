use valkey_module::alloc::ValkeyAlloc;
use valkey_module::raw::{KeyType, RedisModuleStreamID};
use valkey_module::{
    valkey_module, Context, NextArg, ValkeyError, ValkeyResult, ValkeyString, ValkeyValue,
};

fn stream_read_from(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    let mut args = args.into_iter().skip(1);

    let stream_key = args.next_arg()?;

    let stream = ctx.open_key(&stream_key);
    let key_type = stream.key_type();

    if key_type != KeyType::Stream {
        return Err(ValkeyError::WrongType);
    }

    let mut iter = stream.get_stream_iterator(false)?;
    let element = iter.next();
    let id_to_keep = iter.next().as_ref().map_or_else(
        || RedisModuleStreamID {
            ms: u64::MAX,
            seq: u64::MAX,
        },
        |e| e.id,
    );

    let stream = ctx.open_key_writable(&stream_key);
    stream.trim_stream_by_id(id_to_keep, false)?;
    Ok(match element {
        Some(e) => ValkeyValue::BulkString(format!("{}-{}", e.id.ms, e.id.seq)),
        None => ValkeyValue::Null,
    })
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "stream",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["STREAM_POP", stream_read_from, "write", 1, 1, 1],
    ],
}
