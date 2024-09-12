use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{
    valkey_module, AclPermissions, Context, NextArg, ValkeyError, ValkeyResult, ValkeyString,
    ValkeyValue,
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

//////////////////////////////////////////////////////

valkey_module! {
    name: "acl",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["verify_key_access_for_user", verify_key_access_for_user, "", 0, 0, 0],
        ["get_current_user", get_current_user, "", 0, 0, 0],
    ],
}
