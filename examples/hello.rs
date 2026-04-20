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
    use valkey_module::{Context, ValkeyValue};

    fn test_args(values: &[&str]) -> Vec<ValkeyString> {
        values
            .iter()
            .map(|value| ValkeyString::create_for_test(*value))
            .collect()
    }

    #[test]
    fn hello_mul_returns_inputs_and_product() {
        let reply = hello_mul(&Context::dummy(), test_args(&["hello.mul", "2", "3", "4"])).unwrap();

        assert_eq!(
            reply,
            ValkeyValue::Array(vec![
                ValkeyValue::Integer(2),
                ValkeyValue::Integer(3),
                ValkeyValue::Integer(4),
                ValkeyValue::Integer(24),
            ])
        );
    }

    #[test]
    fn hello_mul_returns_wrong_arity_without_numbers() {
        let err = hello_mul(&Context::dummy(), test_args(&["hello.mul"])).unwrap_err();

        assert!(matches!(err, ValkeyError::WrongArity));
    }

    #[test]
    fn hello_mul_rejects_invalid_integer() {
        let err = hello_mul(&Context::dummy(), test_args(&["hello.mul", "2", "xx"])).unwrap_err();

        assert!(matches!(err, ValkeyError::Str("Couldn't parse as integer")));
    }
}
