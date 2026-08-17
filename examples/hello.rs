use valkey_module::alloc::ValkeyAlloc;
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
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["hello.mul", hello_mul, "", 0, 0, 0],
    ],
}

#[cfg(test)]
mod tests {
    use super::*;
    use valkey_module::test_shims::create_test_args;
    use valkey_module::ValkeyValue;

    #[test]
    fn multiplies_integer_arguments() {
        let result = hello_mul(
            &Context::dummy(),
            create_test_args(&["hello.mul", "2", "3", "4"]),
        )
        .expect("valid integers should be multiplied");

        assert_eq!(
            result,
            ValkeyValue::Array(vec![
                ValkeyValue::Integer(2),
                ValkeyValue::Integer(3),
                ValkeyValue::Integer(4),
                ValkeyValue::Integer(24),
            ])
        );
    }

    #[test]
    fn rejects_missing_arguments() {
        let result = hello_mul(&Context::dummy(), create_test_args(&["hello.mul"]));

        assert!(matches!(result, Err(ValkeyError::WrongArity)));
    }

    #[test]
    fn rejects_non_integer_arguments() {
        let result = hello_mul(
            &Context::dummy(),
            create_test_args(&["hello.mul", "2", "invalid"]),
        );

        assert!(matches!(result, Err(ValkeyError::Str(_))));
    }
}
