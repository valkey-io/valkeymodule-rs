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
        unsafe { raw::RedisModule_DefragAlloc.unwrap()(self.defrag_ctx, ptr) }
    }

    /// # Sets a cursor on the last item defragged so that the next defrag cycle can start fromn that position
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_DefragCursorSet` is missing in redismodule.h
    pub unsafe fn set_cursor(&self, cursor: u64) -> Status {
        let status = unsafe { raw::RedisModule_DefragCursorSet.unwrap()(self.defrag_ctx, cursor) };
        if status as isize == raw::REDISMODULE_OK {
            Status::Ok
        } else {
            Status::Err
        }
    }

    /// # Returns the place we last defragged if applicable.
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_DefragCursorGet` is missing in redismodule.h
    pub unsafe fn get_cursor(&self) -> Option<u64> {
        let mut cursor: u64 = 0;
        let status =
            unsafe { raw::RedisModule_DefragCursorGet.unwrap()(self.defrag_ctx, &mut cursor) };
        if status as isize == raw::REDISMODULE_OK {
            Some(cursor)
        } else {
            None
        }
    }

    /// # Returns true if we have been defragging to long and need to stop
    ///
    /// # Panics
    ///
    /// Will panic if `RedisModule_DefragShouldStop` is missing in redismodule.h
    pub unsafe fn should_stop_defrag(&self) -> bool {
        unsafe { raw::RedisModule_DefragShouldStop.unwrap()(self.defrag_ctx) != 0 }
    }
}
