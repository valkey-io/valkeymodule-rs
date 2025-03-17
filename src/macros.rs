#[macro_export]
macro_rules! redis_command {
    (
        $ctx:expr,
        $command_name:expr,
        $command_handler:expr,
        $command_flags:expr,
        $firstkey:expr,
        $lastkey:expr,
        $keystep:expr
        $(,
            $command_acl_categories:expr
        )?
        ) => {{
        let name = CString::new($command_name).unwrap();
        let flags = CString::new($command_flags).unwrap();
        /////////////////////
        extern "C" fn __do_command(
            ctx: *mut $crate::raw::RedisModuleCtx,
            argv: *mut *mut $crate::raw::RedisModuleString,
            argc: c_int,
        ) -> c_int {
            let context = $crate::Context::new(ctx);
            let args = $crate::decode_args(ctx, argv, argc);
            let response = $command_handler(&context, args);
            context.reply(response.map(|v| v.into())) as c_int
        }

        if unsafe {
            $crate::raw::RedisModule_CreateCommand.unwrap()(
                $ctx,
                name.as_ptr(),
                Some(__do_command),
                flags.as_ptr(),
                $firstkey,
                $lastkey,
                $keystep,
            )
        } == $crate::raw::Status::Err as c_int
        {
            return $crate::raw::Status::Err as c_int;
        }

        $(
            let context = $crate::Context::new($ctx);
            let acl_categories_to_add = CString::new($command_acl_categories).unwrap();
            #[cfg(feature = "min-valkey-compatibility-version-8-0")]
            context.set_acl_category(name.as_ptr(), acl_categories_to_add.as_ptr());
        )?
    }};
}

#[macro_export]
macro_rules! redis_event_handler {
    (
        $ctx: expr,
        $event_type: expr,
        $event_handler: expr
    ) => {{
        extern "C" fn __handle_event(
            ctx: *mut $crate::raw::RedisModuleCtx,
            event_type: c_int,
            event: *const c_char,
            key: *mut $crate::raw::RedisModuleString,
        ) -> c_int {
            let context = $crate::Context::new(ctx);

            let redis_key = $crate::ValkeyString::string_as_slice(key);
            let event_str = unsafe { CStr::from_ptr(event) };
            $event_handler(
                &context,
                $crate::NotifyEvent::from_bits_truncate(event_type),
                event_str.to_str().unwrap(),
                redis_key,
            );

            $crate::raw::Status::Ok as c_int
        }

        let all_available_notification_flags = $crate::raw::get_keyspace_notification_flags_all();
        let available_wanted_notification_flags = $event_type.intersection(all_available_notification_flags);
        if !all_available_notification_flags.contains($event_type) {
            let not_supported = $event_type.difference(all_available_notification_flags);
            $crate::Context::new($ctx).log_notice(&format!(
                "These event notification flags set aren't supported: {not_supported:?}. These flags will be used: {available_wanted_notification_flags:?}"
            ));
        }

        if !available_wanted_notification_flags.is_empty() && unsafe {
            $crate::raw::RedisModule_SubscribeToKeyspaceEvents.unwrap()(
                $ctx,
                available_wanted_notification_flags.bits(),
                Some(__handle_event),
            )
        } == $crate::raw::Status::Err as c_int
        {
            return $crate::raw::Status::Err as c_int;
        }
    }};
}

/// Defines a Valkey module.
///
/// It registers the defined module, sets it up and initialises properly,
/// registers all the commands and types.
#[macro_export]
macro_rules! valkey_module {
    (
        name: $module_name:expr,
        version: $module_version:expr,
        /// Global allocator for the valkey module defined.
        /// In most of the cases, the Valkey allocator ([crate::alloc::ValkeyAlloc])
        /// should be used.
        allocator: ($allocator_type:ty, $allocator_init:expr),
        data_types: [
            $($data_type:ident),* $(,)*
        ],
        $(init: $init_func:ident,)* $(,)*
        $(deinit: $deinit_func:ident,)* $(,)*
        $(info: $info_func:ident,)?
        $(auth: [
            $($auth_callback:expr),* $(,)*
        ],)?
        $(acl_categories: [
            $($acl_category:expr),* $(,)*
        ])?
        commands: [
            $([
                $name:expr,
                $command:expr,
                $flags:expr,
                $firstkey:expr,
                $lastkey:expr,
                $keystep:expr
                $(,
                    $command_acl_categories:expr
                )?
            ]),* $(,)?
        ] $(,)*
        $(event_handlers: [
            $([
                $(@$event_type:ident) +:
                $event_handler:expr
            ]),* $(,)*
        ] $(,)* )?
        $(configurations: [
            $(i64:[$([
                $i64_configuration_name:expr,
                $i64_configuration_val:expr,
                $i64_default:expr,
                $i64_min:expr,
                $i64_max:expr,
                $i64_flags_options:expr,
                $i64_on_changed:expr $(, $i64_on_set:expr)?
            ]),* $(,)*],)?
            $(string:[$([
                $string_configuration_name:expr,
                $string_configuration_val:expr,
                $string_default:expr,
                $string_flags_options:expr,
                $string_on_changed:expr $(, $string_on_set:expr)?
            ]),* $(,)*],)?
            $(bool:[$([
                $bool_configuration_name:expr,
                $bool_configuration_val:expr,
                $bool_default:expr,
                $bool_flags_options:expr,
                $bool_on_changed:expr $(, $bool_on_set:expr)?
            ]),* $(,)*],)?
            $(enum:[$([
                $enum_configuration_name:expr,
                $enum_configuration_val:expr,
                $enum_default:expr,
                $enum_flags_options:expr,
                $enum_on_changed:expr $(, $enum_on_set:expr)?
            ]),* $(,)*],)?
            $(module_args_as_configuration:$use_module_args:expr,)?
            $(module_config_get:$module_config_get_command:expr,)?
            $(module_config_set:$module_config_set_command:expr,)?
        ])?
    ) => {
        /// Valkey module allocator.
        #[global_allocator]
        static REDIS_MODULE_ALLOCATOR: $allocator_type = $allocator_init;

        // The old-style info command handler, if specified.
        $(
            #[valkey_module_macros::info_command_handler]
            #[inline]
            fn module_info(ctx: &InfoContext, for_crash_report: bool) -> ValkeyResult<()> {
                $info_func(ctx, for_crash_report);

                Ok(())
            }
        )?

        extern "C" fn __info_func(
            ctx: *mut $crate::raw::RedisModuleInfoCtx,
            for_crash_report: i32,
        ) {
            $crate::basic_info_command_handler(&$crate::InfoContext::new(ctx), for_crash_report == 1);
        }

        #[no_mangle]
        #[allow(non_snake_case)]
        pub unsafe extern "C" fn RedisModule_OnLoad(
            ctx: *mut $crate::raw::RedisModuleCtx,
            argv: *mut *mut $crate::raw::RedisModuleString,
            argc: std::os::raw::c_int,
        ) -> std::os::raw::c_int {
            use std::os::raw::{c_int, c_char};
            use std::ffi::{CString, CStr};

            use $crate::raw;
            use $crate::ValkeyString;
            use $crate::server_events::register_server_events;
            use $crate::configuration::register_i64_configuration;
            use $crate::configuration::register_string_configuration;
            use $crate::configuration::register_bool_configuration;
            use $crate::configuration::register_enum_configuration;
            use $crate::configuration::module_config_get;
            use $crate::configuration::module_config_set;
            use $crate::configuration::get_i64_default_config_value;
            use $crate::configuration::get_string_default_config_value;
            use $crate::configuration::get_bool_default_config_value;
            use $crate::configuration::get_enum_default_config_value;

            // We use a statically sized buffer to avoid allocating.
            // This is needed since we use a custom allocator that relies on the Valkey allocator,
            // which isn't yet ready at this point.
            let mut name_buffer = [0; 64];
            unsafe {
                std::ptr::copy(
                    $module_name.as_ptr(),
                    name_buffer.as_mut_ptr(),
                    $module_name.len(),
                );
            }

            let module_version = $module_version as c_int;

            // This block of code means that when Modules are compiled without the "use-redismodule-api" feature flag,
            // we expect that ValkeyModule_Init should succeed. We do not YET utilize the ValkeyModule_Init invocation
            // because the valkeymodule-rs still references RedisModule_* APIs for calls to the server.
            #[cfg(not(feature = "use-redismodule-api"))]
            {
                let status = unsafe {
                    raw::Export_ValkeyModule_Init(
                        ctx as *mut raw::ValkeyModuleCtx,
                        name_buffer.as_ptr().cast::<c_char>(),
                        module_version,
                        raw::VALKEYMODULE_APIVER_1 as c_int,
                    )
                };
                if status == raw::Status::Err as c_int {
                    return raw::Status::Err as c_int;
                }
            }

            // For now, we need to initialize through RM_Init because several occurances are still using RedisModule_* APIs.
            // Once we change every single Module API to be ValkeyModule_* (when the feature flag is not provided), we can
            // update this block (invocation of RM_Init) to only be executed when the "use-redismodule-api" is provided.
            let status = unsafe {
                raw::Export_RedisModule_Init(
                    ctx,
                    name_buffer.as_ptr().cast::<c_char>(),
                    module_version,
                    raw::REDISMODULE_APIVER_1 as c_int,
                )
            };
            if status == raw::Status::Err as c_int {
                return raw::Status::Err as c_int;
            }

            let context = $crate::Context::new(ctx);
            unsafe {
                let _ = $crate::MODULE_CONTEXT.set_context(&context);
            }
            let args = $crate::decode_args(ctx, argv, argc);

            $(
                if (&$data_type).create_data_type(ctx).is_err() {
                    return raw::Status::Err as c_int;
                }
            )*

            $(
                $(
                    #[cfg(feature = "min-valkey-compatibility-version-8-0")]
                    context.add_acl_category($acl_category);
                )*
            )?

            $(
                $crate::redis_command!(ctx, $name, $command, $flags, $firstkey, $lastkey, $keystep $(, $command_acl_categories)?);
            )*

            if $crate::commands::register_commands(&context) == raw::Status::Err {
                return raw::Status::Err as c_int;
            }

            $(
                $(
                    $crate::redis_event_handler!(ctx, $(raw::NotifyEvent::$event_type |)+ raw::NotifyEvent::empty(), $event_handler);
                )*
            )?

            $(
                $(
                    $(
                        let default = if $use_module_args {
                            match get_i64_default_config_value(&args, $i64_configuration_name, $i64_default) {
                                Ok(v) => v,
                                Err(e) => {
                                    context.log_warning(&format!("{e}"));
                                    return raw::Status::Err as c_int;
                                }
                            }
                        } else {
                            $i64_default
                        };
                        let mut use_fallback = true;
                        $(
                            use_fallback = false;
                            register_i64_configuration(&context, $i64_configuration_name, $i64_configuration_val, default, $i64_min, $i64_max, $i64_flags_options, $i64_on_changed, $i64_on_set);
                        )?
                        if (use_fallback) {
                            register_i64_configuration(&context, $i64_configuration_name, $i64_configuration_val, default, $i64_min, $i64_max, $i64_flags_options, $i64_on_changed, None);
                        }
                    )*
                )?
                $(
                    $(
                        let default = if $use_module_args {
                            match get_string_default_config_value(&args, $string_configuration_name, $string_default) {
                                Ok(v) => v,
                                Err(e) => {
                                    context.log_warning(&format!("{e}"));
                                    return raw::Status::Err as c_int;
                                }
                            }
                        } else {
                            $string_default
                        };
                        let mut use_fallback = true;
                        $(
                            use_fallback = false;
                            register_string_configuration(&context, $string_configuration_name, $string_configuration_val, default, $string_flags_options, $string_on_changed, $string_on_set);
                        )?
                        if (use_fallback) {
                            register_string_configuration(&context, $string_configuration_name, $string_configuration_val, default, $string_flags_options, $string_on_changed, None);
                        }
                    )*
                )?
                $(
                    $(
                        let default = if $use_module_args {
                            match get_bool_default_config_value(&args, $bool_configuration_name, $bool_default) {
                                Ok(v) => v,
                                Err(e) => {
                                    context.log_warning(&format!("{e}"));
                                    return raw::Status::Err as c_int;
                                }
                            }
                        } else {
                            $bool_default
                        };
                        let mut use_fallback = true;
                        $(
                            use_fallback = false;
                            register_bool_configuration(&context, $bool_configuration_name, $bool_configuration_val, default, $bool_flags_options, $bool_on_changed, $bool_on_set);
                        )?
                        if (use_fallback) {
                            register_bool_configuration(&context, $bool_configuration_name, $bool_configuration_val, default, $bool_flags_options, $bool_on_changed, None);
                        }
                    )*
                )?
                $(
                    $(
                        let default = if $use_module_args {
                            match get_enum_default_config_value(&args, $enum_configuration_name, $enum_default) {
                                Ok(v) => v,
                                Err(e) => {
                                    context.log_warning(&format!("{e}"));
                                    return raw::Status::Err as c_int;
                                }
                            }
                        } else {
                            $enum_default
                        };
                        let mut use_fallback = true;
                        $(
                            use_fallback = false;
                            register_enum_configuration(&context, $enum_configuration_name, $enum_configuration_val, default.clone(), $enum_flags_options, $enum_on_changed, $enum_on_set);
                        )?
                        if (use_fallback) {
                            register_enum_configuration(&context, $enum_configuration_name, $enum_configuration_val, default.clone(), $enum_flags_options, $enum_on_changed, None);
                        }
                    )*
                )?
                raw::RedisModule_LoadConfigs.unwrap()(ctx);

                $(
                    $crate::redis_command!(ctx, $module_config_get_command, |ctx, args: Vec<RedisString>| {
                        module_config_get(ctx, args, $module_name)
                    }, "", 0, 0, 0);
                )?

                $(
                    $crate::redis_command!(ctx, $module_config_set_command, |ctx, args: Vec<RedisString>| {
                        module_config_set(ctx, args, $module_name)
                    }, "", 0, 0, 0);
                )?
            )?

            raw::register_info_function(ctx, Some(__info_func));

            $(
                $crate::valkey_module_auth!(ctx, $($auth_callback),*);
            )?

            if let Err(e) = register_server_events(&context) {
                context.log_warning(&format!("{e}"));
                return raw::Status::Err as c_int;
            }

            $(
                if $init_func(&context, &args) == $crate::Status::Err {
                    return $crate::Status::Err as c_int;
                }
            )*

            raw::Status::Ok as c_int
        }

        #[no_mangle]
        #[allow(non_snake_case)]
        pub extern "C" fn RedisModule_OnUnload(
            ctx: *mut $crate::raw::RedisModuleCtx
        ) -> std::os::raw::c_int {
            use std::os::raw::c_int;

            let context = $crate::Context::new(ctx);
            $(
                if $deinit_func(&context) == $crate::Status::Err {
                    return $crate::Status::Err as c_int;
                }
            )*

            $crate::raw::Status::Ok as c_int
        }
    }
}

#[macro_export]
macro_rules! valkey_module_auth {
    ($ctx:expr, $($auth_callback:expr),* $(,)*) => {
        $(
            {   // New scope for each callback
                extern "C" fn __do_auth(
                    ctx: *mut $crate::raw::RedisModuleCtx,
                    username: *mut $crate::raw::RedisModuleString,
                    password: *mut $crate::raw::RedisModuleString,
                    err: *mut *mut $crate::raw::RedisModuleString,
                ) -> std::os::raw::c_int {
                    let context = $crate::Context::new(ctx);
                    let ctx_ptr = unsafe { std::ptr::NonNull::new_unchecked(ctx) };
                    let username = $crate::ValkeyString::new(Some(ctx_ptr), username);
                    let password = $crate::ValkeyString::new(Some(ctx_ptr), password);

                    match $auth_callback(&context, username, password) {
                        Ok(result) => result,
                        Err(e) => {
                            let error_msg = $crate::ValkeyString::create(None, e.to_string().as_str());
                            unsafe { *err = error_msg.into_raw() };
                            $crate::AUTH_HANDLED
                        }
                    }
                }

                unsafe {
                    if let Some(register_auth) = $crate::raw::RedisModule_RegisterAuthCallback {
                        register_auth($ctx, Some(__do_auth));
                    }
                }
            }
        )*
    };
}
