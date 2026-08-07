//! Low-overhead module resource accounting.
//!
//! The memory counters track Rust heap allocation requests that pass through
//! [`crate::alloc::ValkeyAlloc`]. They do not include Redis keyspace memory,
//! Redis module API objects, replies, socket buffers, thread stacks, mmap
//! regions, or allocations made directly through other FFI APIs.

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Snapshot {
    /// Bytes currently attributed to Rust allocations made through ValkeyAlloc.
    pub memory_current_bytes: u64,
    /// High-water mark of memory_current_bytes.
    ///
    /// This can be temporarily overstated during Rust realloc/growth paths
    /// because ValkeyAlloc does not override GlobalAlloc::realloc in phase 1.
    pub memory_peak_bytes: u64,
    /// Allocation events observed by the Rust global allocator.
    pub alloc_count: u64,
    /// Free events observed by the Rust global allocator.
    pub free_count: u64,
    /// Derived count of allocation events that have not been matched by free events.
    pub live_allocations: u64,
    /// Number of saturating accounting corrections applied to avoid counter wraparound.
    pub memory_accounting_errors: u64,
}

pub struct ModuleStats {
    memory_current_bytes: AtomicU64,
    memory_peak_bytes: AtomicU64,
    alloc_count: AtomicU64,
    free_count: AtomicU64,
    memory_accounting_errors: AtomicU64,
}

impl ModuleStats {
    pub const fn new() -> Self {
        Self {
            memory_current_bytes: AtomicU64::new(0),
            memory_peak_bytes: AtomicU64::new(0),
            alloc_count: AtomicU64::new(0),
            free_count: AtomicU64::new(0),
            memory_accounting_errors: AtomicU64::new(0),
        }
    }

    pub fn add_alloc(&self, bytes: u64) {
        self.alloc_count.fetch_add(1, Ordering::Relaxed);
        let (current, overflowed) = add_saturating(&self.memory_current_bytes, bytes);
        if overflowed {
            self.memory_accounting_errors
                .fetch_add(1, Ordering::Relaxed);
        }
        self.update_peak(current);
    }

    pub fn add_free(&self, bytes: u64) {
        self.free_count.fetch_add(1, Ordering::Relaxed);
        if subtract_saturating(&self.memory_current_bytes, bytes) {
            self.memory_accounting_errors
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let alloc_count = self.alloc_count.load(Ordering::Relaxed);
        let free_count = self.free_count.load(Ordering::Relaxed);

        Snapshot {
            memory_current_bytes: self.memory_current_bytes.load(Ordering::Relaxed),
            memory_peak_bytes: self.memory_peak_bytes.load(Ordering::Relaxed),
            alloc_count,
            free_count,
            live_allocations: alloc_count.saturating_sub(free_count),
            memory_accounting_errors: self.memory_accounting_errors.load(Ordering::Relaxed),
        }
    }

    fn update_peak(&self, current: u64) {
        let mut peak = self.memory_peak_bytes.load(Ordering::Relaxed);
        while current > peak
            && self
                .memory_peak_bytes
                .compare_exchange_weak(peak, current, Ordering::Relaxed, Ordering::Relaxed)
                .is_err()
        {
            peak = self.memory_peak_bytes.load(Ordering::Relaxed);
        }
    }
}

impl Default for ModuleStats {
    fn default() -> Self {
        Self::new()
    }
}

static STATS: ModuleStats = ModuleStats::new();

#[cfg(feature = "enable-usage-tracking")]
pub(crate) fn add_alloc(bytes: u64) {
    STATS.add_alloc(bytes);
}

#[cfg(feature = "enable-usage-tracking")]
pub(crate) fn add_free(bytes: u64) {
    STATS.add_free(bytes);
}

pub fn snapshot() -> Snapshot {
    STATS.snapshot()
}

pub fn snapshot_text() -> String {
    let snapshot = snapshot();
    format!(
        "memory_current_bytes={}\nmemory_peak_bytes={}\nalloc_count={}\nfree_count={}\nlive_allocations={}\nmemory_accounting_errors={}\n",
        snapshot.memory_current_bytes,
        snapshot.memory_peak_bytes,
        snapshot.alloc_count,
        snapshot.free_count,
        snapshot.live_allocations,
        snapshot.memory_accounting_errors,
    )
}

fn add_saturating(value: &AtomicU64, amount: u64) -> (u64, bool) {
    let mut current = value.load(Ordering::Relaxed);

    loop {
        let (next, overflowed) = current.overflowing_add(amount);
        let next = if overflowed { u64::MAX } else { next };

        match value.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return (next, overflowed),
            Err(actual) => current = actual,
        }
    }
}

fn subtract_saturating(value: &AtomicU64, amount: u64) -> bool {
    let mut current = value.load(Ordering::Relaxed);

    loop {
        let underflowed = amount > current;
        let next = if underflowed { 0 } else { current - amount };

        match value.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return underflowed,
            Err(actual) => current = actual,
        }
    }
}
