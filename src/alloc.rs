use std::alloc::{GlobalAlloc, Layout};

use crate::raw;

/// Panics with a message without using an allocator.
/// Useful when using the allocator should be avoided or it is
/// inaccessible. The default [std::panic] performs allocations and so
/// will cause a double panic without a meaningful message if the
/// allocator can't be used. This function makes sure we can panic with
/// a reasonable message even without the allocator working.
fn allocation_free_panic(message: &'static str) -> ! {
    use std::os::unix::io::AsRawFd;

    let _ = nix::unistd::write(std::io::stderr().as_raw_fd(), message.as_bytes());

    std::process::abort();
}

const VALKEY_ALLOCATOR_NOT_AVAILABLE_MESSAGE: &str =
    "Critical error: the Valkey Allocator isn't available.\n";

/// Defines the Valkey allocator. This allocator delegates the allocation
/// and deallocation tasks to the Valkey server when available, otherwise
/// it panics.
#[derive(Copy, Clone)]
pub struct ValkeyAlloc;

unsafe impl GlobalAlloc for ValkeyAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        /*
         * To make sure the memory allocation by Valkey is aligned to the according to the layout,
         * we need to align the size of the allocation to the layout.
         *
         * "Memory is conceptually broken into equal-sized chunks,
         * where the chunk size is a power of two that is greater than the page size.
         * Chunks are always aligned to multiples of the chunk size.
         * This alignment makes it possible to find metadata for user objects very quickly."
         *
         * From: https://linux.die.net/man/3/jemalloc
         */
        if cfg!(feature = "enable-system-alloc") {
            return std::alloc::System.alloc(layout);
        }
        let size = (layout.size() + layout.align() - 1) & (!(layout.align() - 1));

        match raw::RedisModule_Alloc {
            Some(alloc) => alloc(size).cast(),
            None => allocation_free_panic(VALKEY_ALLOCATOR_NOT_AVAILABLE_MESSAGE),
        }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        /*
         * To make sure the memory allocation by Valkey is aligned to the according to the layout,
         * we need to align the size of the allocation to the layout.
         *
         * "Memory is conceptually broken into equal-sized chunks,
         * where the chunk size is a power of two that is greater than the page size.
         * Chunks are always aligned to multiples of the chunk size.
         * This alignment makes it possible to find metadata for user objects very quickly."
         *
         * From: https://linux.die.net/man/3/jemalloc
         */
        if cfg!(feature = "enable-system-alloc") {
            return std::alloc::System.alloc_zeroed(layout);
        }
        let size = (layout.size() + layout.align() - 1) & (!(layout.align() - 1));

        match raw::RedisModule_Calloc {
            Some(calloc) => calloc(size, 1).cast(),
            None => allocation_free_panic(VALKEY_ALLOCATOR_NOT_AVAILABLE_MESSAGE),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if cfg!(feature = "enable-system-alloc") {
            return std::alloc::System.dealloc(ptr, layout);
        }
        match raw::RedisModule_Free {
            Some(f) => f(ptr.cast()),
            None => allocation_free_panic(VALKEY_ALLOCATOR_NOT_AVAILABLE_MESSAGE),
        };
    }
}
