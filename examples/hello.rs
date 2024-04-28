use valkey_module::{valkey_module, Context, ValkeyError, ValkeyResult, ValkeyString};

fn hello_mul(_: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() < 2 {
        return Err(ValkeyError::WrongArity);
    }

    let nums = args
        .into_iter()
        .skip(1)
        .map(|s| s.parse_integer())
        .collect::<Result<Vec<i64>, ValkeyError>>()?;

    let product = nums.iter().product();

    let mut response = nums;
    response.push(product);

    Ok(response.into())
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "hello",
    version: 1,
    allocator: (valkey_module::alloc::ValkeyAlloc, valkey_module::alloc::ValkeyAlloc),
    data_types: [],
    commands: [
        ["hello.mul", hello_mul, "", 0, 0, 0],
    ],
}
