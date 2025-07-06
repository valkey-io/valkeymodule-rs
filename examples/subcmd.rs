use std::collections::BTreeMap;
use valkey_module::redisvalue::ValkeyValueKey;
use valkey_module::{
    alloc::ValkeyAlloc, valkey_module, Context, NextArg, ValkeyError, ValkeyResult, ValkeyString,
    ValkeyValue,
};

// top level command doesn't have logic, simply acts as a wrapper for subcommands
fn cmd1(_ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() == 1 {
        // display help as default subcommand
        return help_subcmd();
    };
    let mut args = args.into_iter().skip(1);
    let subcmd = args.next_string()?;
    let args: Vec<ValkeyString> = args.collect();
    match subcmd.to_lowercase().as_str() {
        "s1" => sub1(args),
        // more subcommands can be added here
        "info" => info_subcmd(args),
        "help" => help_subcmd(),
        _ => Err(ValkeyError::Str("invalid subcommand")),
    }
}

/// can be called either with `cmd1 help` or just `cmd1`
fn help_subcmd() -> ValkeyResult {
    let output = vec![
        ValkeyValue::SimpleString("cmd1 - top level command".into()),
        ValkeyValue::SimpleString("cmd1 s1 - first level subcommand".into()),
        ValkeyValue::SimpleString("cmd1 s1 s1 - second level command".into()),
        ValkeyValue::SimpleString("cmd1 s1 s1 s1 - third level command".into()),
        ValkeyValue::SimpleString("cmd1 help - display this message".into()),
    ];
    Ok(output.into())
}

// custom info subcommand, can be called with `cmd1 info`
fn info_subcmd(args: Vec<ValkeyString>) -> ValkeyResult {
    let section = args.into_iter().next_str().unwrap_or("all");

    let sections = [
        ("key", ValkeyValue::SimpleString("value".into())),
        ("integer", ValkeyValue::Integer(1)),
        ("float", ValkeyValue::Float(1.1)),
        ("bool", ValkeyValue::Bool(true)),
        (
            "array",
            ValkeyValue::Array(vec!["a", "b", "c"].into_iter().map(|s| s.into()).collect()),
        ),
        (
            "ordered-map",
            ValkeyValue::OrderedMap(BTreeMap::from([
                ("key1".into(), "value1".into()),
                ("key2".into(), "value2".into()),
            ])),
        ),
        (
            "ordered-set",
            ValkeyValue::OrderedSet(vec!["x", "y", "z"].into_iter().map(|s| s.into()).collect()),
        ),
        // add more sections here as needed
    ];

    let mut output: BTreeMap<ValkeyValueKey, ValkeyValue> = BTreeMap::new();
    for (key, value) in sections {
        if section == "all" || section == key {
            output.insert(key.into(), value);
        }
    }
    Ok(ValkeyValue::OrderedMap(output))
}
fn sub1(args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() == 0 {
        // return if no args for subcmd are passed in, additional bizlogic can be added here
        return Ok("sub1".into());
    };
    let mut args = args.into_iter();
    let subcmd = args.next_string()?;
    let args: Vec<ValkeyString> = args.collect();
    match subcmd.to_lowercase().as_str() {
        "s1" => sub11(args),
        // more subcommands can be added here
        _ => Ok("sub1".into()),
    }
}

fn sub11(args: Vec<ValkeyString>) -> ValkeyResult {
    if args.len() == 0 {
        // return if no args for subcmd are passed in, additional bizlogic can be added here
        return Ok("sub11".into());
    };
    let mut args = args.into_iter();
    let subcmd = args.next_string()?;
    let args: Vec<ValkeyString> = args.collect();
    match subcmd.to_lowercase().as_str() {
        "s1" => sub111(args),
        // more subcommands can be added here
        _ => Ok("sub11".into()),
    }
}

fn sub111(_args: Vec<ValkeyString>) -> ValkeyResult {
    // add bizlogic here
    Ok("sub111".into())
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "subcmd",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["cmd1", cmd1, "", 0, 0, 0],
    ],
}
