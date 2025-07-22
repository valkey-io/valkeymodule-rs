use std::collections::{BTreeMap, HashMap};
use std::os::raw::{c_int, c_void};
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::logging::log_notice;
use valkey_module::native_types::ValkeyType;
use valkey_module::redisvalue::ValkeyValueKey;
use valkey_module::{
    raw, valkey_module, Context, NextArg, RedisModuleIO, RedisModuleTypeMethods, ValkeyError,
    ValkeyResult, ValkeyString, ValkeyValue, REDISMODULE_TYPE_METHOD_VERSION,
};

// example data type with a string, number, vector, and map, also has callback for save, load and free
// deliverately made the code verbose to show the steps clearly
#[derive(Debug, Default)]
struct MyType {
    my_string: String,
    my_number: i64,
    my_vec: Vec<String>,
    my_map: HashMap<String, String>,
}
impl MyType {
    // used for outputting the data in the command response
    /*
    127.0.0.1:6379> my-get key1
    1# "my_map" =>
        1# "k1" => "v1"
        2# "k2" => "v2"
    2# "my_number" => (integer) 1
    3# "my_string" => "a"
    4# "my_vec" =>
        1) "a"
        2) "b"
     */
    fn to_btreemap(&self) -> BTreeMap<ValkeyValueKey, ValkeyValue> {
        let mut outut = BTreeMap::new();
        outut.insert("my_string".into(), self.my_string.clone().into());
        outut.insert("my_number".into(), self.my_number.into());
        let my_vec = self
            .my_vec
            .iter()
            .map(|s| s.into())
            .collect::<Vec<ValkeyValue>>();
        outut.insert("my_vec".into(), my_vec.into());
        let my_map = self
            .my_map
            .iter()
            .map(|(k, v)| (k.clone().into(), v.clone().into()))
            .collect::<BTreeMap<ValkeyValueKey, ValkeyValue>>();
        outut.insert("my_map".into(), my_map.into());
        outut
    }
}

static MY_TYPE: ValkeyType = ValkeyType::new(
    "mytype789",
    0,
    RedisModuleTypeMethods {
        version: REDISMODULE_TYPE_METHOD_VERSION as u64,
        rdb_load: Some(rdb_load),
        rdb_save: Some(rdb_save),
        aof_rewrite: None,
        free: Some(free),
        mem_usage: None,
        digest: None,
        aux_load: None,
        aux_save: None,
        aux_save_triggers: 0,
        free_effort: None,
        unlink: None,
        copy: None,
        defrag: None,
        free_effort2: None,
        unlink2: None,
        copy2: None,
        mem_usage2: None,
        aux_save2: None,
    },
);

// called when the value is no longer needed, Valkey will crash when key is deleted if this is not defined.
extern "C" fn free(value: *mut c_void) {
    if value.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(value.cast::<MyType>())) }
}

// called when data is saved to RDB file
extern "C" fn rdb_save(rdb: *mut RedisModuleIO, value: *mut c_void) {
    if value.is_null() || rdb.is_null() {
        return;
    }
    let item = unsafe { &*value.cast::<MyType>() };
    // save string number, vector, and map
    raw::save_string(rdb, item.my_string.as_str());
    raw::save_signed(rdb, item.my_number);
    // save the size of the vector
    raw::save_unsigned(rdb, item.my_vec.len() as u64);
    // loop through the vector and save each string
    for str in &item.my_vec {
        raw::save_string(rdb, str.as_str());
    }
    // save the size of the map
    raw::save_unsigned(rdb, item.my_map.len() as u64);
    // loop through the map and save each key-value pair
    for (key, value) in &item.my_map {
        raw::save_string(rdb, key.as_str());
        raw::save_string(rdb, value.as_str());
    }
}

// called when data is loaded from RDB file
extern "C" fn rdb_load(rdb: *mut RedisModuleIO, _encver: c_int) -> *mut c_void {
    if rdb.is_null() {
        return std::ptr::null_mut();
    }
    // load string, number, vector, and map.  Must be done in the same order as saved.
    let my_string = match raw::load_string(rdb) {
        Ok(tmp) => tmp.to_string(),
        Err(err) => {
            log_notice(&format!("rdb_load my_string error: {}", err));
            return std::ptr::null_mut();
        }
    };
    let my_number = match raw::load_signed(rdb) {
        Ok(tmp) => tmp,
        Err(err) => {
            log_notice(&format!("rdb_load my_number error: {}", err));
            return std::ptr::null_mut();
        }
    };
    // load the size of the vector
    let vec_size = match raw::load_unsigned(rdb) {
        Ok(tmp) => tmp as usize,
        Err(err) => {
            log_notice(&format!("rdb_load vec_size error: {}", err));
            return std::ptr::null_mut();
        }
    };
    // loop through the vector and load each string, specifying the capacity to optimize memory allocation
    let mut my_vec = Vec::with_capacity(vec_size);
    for count in 0..vec_size {
        match raw::load_string(rdb) {
            Ok(tmp) => my_vec.push(tmp.to_string()),
            Err(err) => {
                log_notice(&format!("rdb_load my_vec error: {}, count: {}", err, count));
                return std::ptr::null_mut();
            }
        }
    }
    // load the size of the map
    let map_size = match raw::load_unsigned(rdb) {
        Ok(tmp) => tmp as usize,
        Err(err) => {
            log_notice(&format!("rdb_load map_size error: {}", err));
            return std::ptr::null_mut();
        }
    };
    // loop through the map and load each key-value pair, specifying the capacity to optimize memory allocation
    let mut my_map = HashMap::with_capacity(map_size);
    for count in 0..map_size {
        let key = match raw::load_string(rdb) {
            Ok(tmp) => tmp.to_string(),
            Err(err) => {
                log_notice(&format!(
                    "rdb_load my_map key error: {}, count: {}",
                    err, count
                ));
                return std::ptr::null_mut();
            }
        };
        let value = match raw::load_string(rdb) {
            Ok(tmp) => tmp.to_string(),
            Err(err) => {
                log_notice(&format!(
                    "rdb_load my_map value error: {}, count: {}",
                    err, count
                ));
                return std::ptr::null_mut();
            }
        };
        my_map.insert(key, value);
    }
    let my_type = MyType {
        my_string,
        my_number,
        my_vec,
        my_map,
    };
    Box::into_raw(Box::new(my_type)) as *mut c_void
}

// command to get the value of MyType
fn my_get(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    let key_arg = args.into_iter().nth(1).ok_or(ValkeyError::WrongArity)?;
    let key = ctx.open_key(&key_arg);
    let current_value = key.get_value::<MyType>(&MY_TYPE)?;
    match current_value {
        Some(tmp) => Ok(tmp.to_btreemap().into()),
        None => Ok(ValkeyValue::Null.into()),
    }
}

// command to set the string value of MyType
fn my_set_string(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    let mut args = args.into_iter().skip(1);
    if args.len() != 2 {
        return Err(ValkeyError::WrongArity);
    }
    let key_arg = args.next_arg()?;
    let value_arg = args.next_string()?;
    let key = ctx.open_key_writable(&key_arg);
    let current_value = key.get_value::<MyType>(&MY_TYPE)?;
    match current_value {
        Some(tmp) => {
            // update existing my_string
            tmp.my_string = value_arg;
        }
        None => {
            // create new key
            let my_type = MyType {
                my_string: value_arg.to_string(),
                ..Default::default()
            };
            key.set_value(&MY_TYPE, my_type)?;
        }
    }
    Ok("OK".into())
}

// command to set the number value of MyType
fn my_set_number(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    let mut args = args.into_iter().skip(1);
    if args.len() != 2 {
        return Err(ValkeyError::WrongArity);
    }
    let key_arg = args.next_arg()?;
    let value_arg = args.next_i64()?;
    let key = ctx.open_key_writable(&key_arg);
    let current_value = key.get_value::<MyType>(&MY_TYPE)?;
    match current_value {
        Some(tmp) => {
            // update existing my_string
            tmp.my_number = value_arg;
        }
        None => {
            // create new key
            let my_type = MyType {
                my_number: value_arg,
                ..Default::default()
            };
            key.set_value(&MY_TYPE, my_type)?;
        }
    }
    Ok("OK".into())
}

// command to push a string to my_vec in MyType
fn my_vec_push(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    let mut args = args.into_iter().skip(1);
    if args.len() != 2 {
        return Err(ValkeyError::WrongArity);
    }
    let key_arg = args.next_arg()?;
    let value_arg = args.next_string()?;
    let key = ctx.open_key_writable(&key_arg);
    let current_value = key.get_value::<MyType>(&MY_TYPE)?;
    match current_value {
        Some(tmp) => {
            // add new value to the vector
            tmp.my_vec.push(value_arg.to_string());
        }
        None => {
            // create new key
            let my_type = MyType {
                my_vec: vec![value_arg.to_string()],
                ..Default::default()
            };
            key.set_value(&MY_TYPE, my_type)?;
        }
    }
    Ok("OK".into())
}

// command to insert a key-value pair into my_map in MyType
fn my_map_insert(ctx: &Context, args: Vec<ValkeyString>) -> ValkeyResult {
    let mut args = args.into_iter().skip(1);
    if args.len() != 3 {
        return Err(ValkeyError::WrongArity);
    }
    let key_arg = args.next_arg()?;
    let value_k_arg = args.next_string()?;
    let value_v_arg = args.next_string()?;
    let key = ctx.open_key_writable(&key_arg);
    let current_value = key.get_value::<MyType>(&MY_TYPE)?;
    match current_value {
        Some(tmp) => {
            // update existing map with k/v pair
            tmp.my_map
                .insert(value_k_arg.to_string(), value_v_arg.to_string());
        }
        None => {
            // create new key
            let my_type = MyType {
                my_map: HashMap::from([(value_k_arg.to_string(), value_v_arg.to_string())]),
                ..Default::default()
            };
            key.set_value(&MY_TYPE, my_type)?;
        }
    }
    Ok("OK".into())
}

//////////////////////////////////////////////////////

valkey_module! {
    name: "data_type3",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [
        MY_TYPE,
    ],
    commands: [
        ["my-get", my_get, "readonly", 0, 0, 0],
        ["my-set-string", my_set_string, "write", 0, 0, 0],
        ["my-set-number", my_set_number, "write", 0, 0, 0],
        ["my-vec-push", my_vec_push, "write", 0, 0, 0],
        ["my-map-insert", my_map_insert, "write", 0, 0, 0],
        // add more commands to pop and remove from the vector and map
    ],
}
