use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{
    valkey_module, AclPermissions, Context, NextArg, ValkeyError, ValkeyResult, ValkeyString,
    ValkeyValue, VALKEY_OK,
};

fn verify_key_access_for_user(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    let mut args = args.into_iter().skip(1);
    let user = args.next_arg()?;
    let key = args.next_arg()?;
    let res = ctx.acl_check_key_permission(&user, &key, &AclPermissions::all());
    if let Err(err) = res {
        return Err(ValkeyError::String(format!("Err {err}")));
    }
    Ok(ValkeyValue::SimpleStringStatic("OK"))
}

fn get_current_user(ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    Ok(ValkeyValue::BulkValkeyString(ctx.get_current_user()))
}

fn custom_category(_ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    VALKEY_OK
}
fn custom_categories(_ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    VALKEY_OK
}
fn existing_categories(_ctx: &Context, _args: Vec<ValkeyString>) -> ValkeyResult {
    VALKEY_OK
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "acl",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    acl_categories: [
        "custom_acl_one",
        "custom_acl_two"
    ]
    commands: [
        ["verify_key_access_for_user", verify_key_access_for_user, "", 0, 0, 0],
        ["get_current_user", get_current_user, "", 0, 0, 0],
        ["custom_category", custom_category, "write",  0, 0, 0, "custom_acl_one"],
        ["custom_categories", custom_categories, "", 0, 0, 0, "custom_acl_one custom_acl_two"],
        ["existing_categories", existing_categories, "write", 0, 0, 0, "read fast admin"],
    ],
}
