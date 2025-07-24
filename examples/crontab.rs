use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{LazyLock, RwLock};
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{valkey_module, Context, Status, ValkeyString};
use valkey_module_macros::cron_event_handler;

// struct to hold environment-specific configs, based on the environment name passed in via MODULE LOAD
#[derive(Debug)]
struct EnvConfig {
    cron_fn1_fn2: String,
    cron_fn3: String,
    // add more environment-specific configs here
}
impl EnvConfig {
    pub(crate) fn new(env: &str) -> Self {
        let output = match env {
            "dev" => EnvConfig {
                // 5 and 10 seconds in dev
                cron_fn1_fn2: "*/5 * * * * * *".to_string(),
                cron_fn3: "*/10 * * * * * *".to_string(),
            },
            // more environments can be added here
            _ => EnvConfig {
                // 15 and 30 seconds by default
                cron_fn1_fn2: "*/15 * * * * * *".to_string(),
                cron_fn3: "*/30 * * * * * *".to_string(),
            },
        };
        output
    }
}
// wrapper for EnvConfig
static ENV_CONFIG: LazyLock<RwLock<EnvConfig>> = LazyLock::new(|| RwLock::new(EnvConfig::new("")));

static CRONTAB: LazyLock<HashMap<String, Vec<fn(&Context)>>> = LazyLock::new(|| {
    // access the ENV_CONFIG to get cron expressions for the environment
    let env_config = ENV_CONFIG.read().unwrap();
    let mut output = HashMap::new();
    // map of cron expressions and their corresponding functions
    // using vector allows to run multiple functions at the same interval
    output.insert(
        env_config.cron_fn1_fn2.clone(),
        vec![cron_fn1 as fn(&Context), cron_fn2 as fn(&Context)],
    );
    // every 30 seconds
    output.insert(env_config.cron_fn3.clone(), vec![cron_fn3 as fn(&Context)]);
    output
});

fn cron_fn1(_ctx: &Context) {
    // biz logic here
}

fn cron_fn2(_ctx: &Context) {
    // biz logic here
}

fn cron_fn3(_ctx: &Context) {
    // biz logic here
}

// uses serverCron to execute custom code on schedule
#[cron_event_handler]
fn cron_event_handler(ctx: &Context, _hz: u64) {
    // default hz value is 10 but check what it's currently set
    // read valkey.conf for details
    let hz = match ctx.config_get("hz".to_string()) {
        Ok(tmp) => tmp.to_string().parse::<u64>().unwrap_or(10),
        Err(_) => 10, // default to 10 if config is not set or invalid
    };
    // how many milliseconds between cron events
    let interval = 1000 / hz as i64;
    for (expression, functions) in CRONTAB.iter() {
        // explicitly use unwrap to crash if there are any issues with the cron expression
        let schedule = cron::Schedule::from_str(expression).unwrap();
        let next_time = schedule.upcoming(chrono::Utc).next().unwrap_or_default();
        let now = chrono::Utc::now();
        // check if the next time is within the interval
        if next_time.timestamp_millis() <= now.timestamp_millis() + interval {
            // loop through functions for that interval
            for function in functions {
                function(ctx);
            }
        }
    }
}

fn initialize(ctx: &Context, args: &[ValkeyString]) -> Status {
    // if arg passed in MODULE LOAD use it to set env_name
    let env_name = match args.get(0) {
        Some(tmp) => tmp.to_string(),
        None => "".to_string(),
    };
    // update ENV_CONFIG static variable based on the env_name
    let mut guard = ENV_CONFIG.write().unwrap();
    *guard = EnvConfig::new(env_name.as_str());
    drop(guard);
    // env_name, "dev", ENV_CONFIG: LazyLock(RwLock { data: EnvConfig { cron_fn1_fn2: "*/15 * * * * * *", cron_fn3: "*/30 * * * * * *" }, poisoned: false, .. })
    ctx.log_notice(&format!(
        "env_name, {:?}, ENV_CONFIG: {:?}",
        env_name, ENV_CONFIG
    ));
    Status::Ok
}

valkey_module! {
    name: "crontab",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    init: initialize,
    commands: [
    ],
}
