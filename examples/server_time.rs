use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{valkey_module, Context, ValkeyResult, ValkeyString, ValkeyValue};

fn unix_ms(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ValkeyValue::Integer(ctx.unix_time_millis()))
}

fn monotonic_us(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    // The monotonic counter starts near zero at process start, so casting the
    // u64 return into the signed Integer reply is safe in practice.
    Ok(ValkeyValue::Integer(ctx.monotonic_micros() as i64))
}

valkey_module! {
    name: "server_time",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["server_time.unix_ms", unix_ms, "readonly fast", 0, 0, 0],
        ["server_time.monotonic_us", monotonic_us, "readonly fast", 0, 0, 0],
    ],
}
