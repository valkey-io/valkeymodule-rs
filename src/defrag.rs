use std::os::raw::c_void;

use crate::{raw, Status};

/// `Defrag` is a high-level rust interface to the Valkey module C API
/// abstracting away the raw C ffi calls.
pub struct Defrag {
    pub defrag_ctx: *mut raw::RedisModuleDefragCtx,
}

impl Defrag {
    pub const fn new(defrag_ctx: *mut raw::RedisModuleDefragCtx) -> Self {
        Self { defrag_ctx }
    }

    /// # Returns a pointer to the new alloction of the data, if no defragmentation was needed a null pointer is returned
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_DefragAlloc` is missing in redismodule.h
    pub unsafe fn alloc(&self, ptr: *mut c_void) -> *mut c_void {
        raw::RedisModule_DefragAlloc.unwrap()(self.defrag_ctx, ptr)
    }

    /// # Sets a cursor on the last item defragged so that on the next defrag cycle, the Module can resume from that position using `get_cursor`.
    /// # Should only be called if `should_stop_defrag` has returned `true` and the defrag callback is about to exit without fully iterating its data type.
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_DefragCursorSet` is missing in redismodule.h
    pub unsafe fn set_cursor(&self, cursor: u64) -> Status {
        let status = raw::RedisModule_DefragCursorSet.unwrap()(self.defrag_ctx, cursor);
        if status as isize == raw::REDISMODULE_OK {
            Status::Ok
        } else {
            Status::Err
        }
    }

    /// # Returns the cursor value that has been previously stored using `set_cursor`
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_DefragCursorGet` is missing in redismodule.h
    pub unsafe fn get_cursor(&self) -> Option<u64> {
        let mut cursor: u64 = 0;
        let status = raw::RedisModule_DefragCursorGet.unwrap()(self.defrag_ctx, &mut cursor);
        if status as isize == raw::REDISMODULE_OK {
            Some(cursor)
        } else {
            None
        }
    }

    /// # Returns true if the engine has been defragging for too long and the Module should need to stop.
    /// # Returns false otherwise for the Module to know it can continue its work.
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_DefragShouldStop` is missing in redismodule.h
    pub unsafe fn should_stop_defrag(&self) -> bool {
        raw::RedisModule_DefragShouldStop.unwrap()(self.defrag_ctx) != 0
    }

    /// # Returns the name of the key being processed.
    /// # If the key name isn't available this will return NULL instead
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_GetKeyNameFromDefragCtx` is missing in redismodule.h
    pub unsafe fn get_key_name_from_defrag_context(&self) -> *const raw::RedisModuleString {
        raw::RedisModule_GetKeyNameFromDefragCtx.unwrap()(self.defrag_ctx)
    }

    /// # Returns the database id of the key that is currently being defragged.
    /// # If this information isn't available it will return -1
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_GetDbIdFromDefragCtx` is missing in redismodule.h
    pub unsafe fn get_db_id_from_defrag_context(&self) -> i32 {
        raw::RedisModule_GetDbIdFromDefragCtx.unwrap()(self.defrag_ctx)
    }
}
