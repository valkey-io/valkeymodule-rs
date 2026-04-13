use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{
    valkey_module, Context, NextArg, ValkeyError, ValkeyResult, ValkeyString,
    ValkeyStringInterface, ValkeyValue,
};

fn mockvalkeystring_greet(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    let mut args = args.into_iter().skip(1);
    let name_arg = args.next_arg()?;
    let greeting = greet(&name_arg)?;
    Ok(ValkeyValue::SimpleString(greeting))
}

/// Build a greeting from a ValkeyString argument.
fn greet(name: &impl ValkeyStringInterface) -> Result<String, ValkeyError> {
    let name_str = name.try_as_str()?;
    Ok(format!("Hello, {name_str}!"))
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "mockvalkeystring",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["mockvalkeystring.greet", mockvalkeystring_greet, "", 0, 0, 0],
    ],
}

//////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use valkey_module::MockValkeyString;

    #[test]
    fn test_greet() {
        let mut mock = MockValkeyString::new();
        mock.expect_try_as_str()
            .returning(|| Ok("Alice".to_string()));
        let result = greet(&mock).unwrap();
        assert_eq!(result, "Hello, Alice!");
    }
}
