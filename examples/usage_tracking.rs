//! Test-only Valkey module for exercising feature-gated Rust heap usage tracking.
//!
//! Each allocation command retains one module-owned data type until
//! `usage_tracking.release` drops it; `usage_tracking.snapshot` exposes the
//! resulting module statistics to the integration test.

use std::{
    mem::size_of_val,
    sync::{atomic::AtomicU64, LazyLock, Mutex},
};

use dashmap::DashMap;
use valkey_module::alloc::ValkeyAlloc;
use valkey_module::{
    module_stats, valkey_module, Context, ValkeyResult, ValkeyString, ValkeyValue,
};

const ALLOCATION_BYTES: usize = 1024 * 1024;
const DASHMAP_ENTRIES: u64 = 64;
const ATOMIC_COUNT: usize = 16 * 1024;

struct CustomTestStruct {
    label: String,
    payload: Vec<u8>,
}

impl CustomTestStruct {
    // Returns the heap-backed data retained by this test structure.
    fn bytes(&self) -> usize {
        self.label.len() + self.payload.len()
    }
}

#[derive(Default)]
struct RetainedAllocations {
    vector: Option<Vec<u8>>,
    dashmap: Option<DashMap<u64, Box<[u8]>>>,
    atomics: Option<Box<[AtomicU64]>>,
    string: Option<String>,
    custom: Option<Box<CustomTestStruct>>,
}

impl RetainedAllocations {
    // Returns the active allocation type and its retained payload size.
    fn active_kind_and_bytes(&self) -> (&'static str, usize) {
        match self {
            Self {
                vector: Some(vector),
                ..
            } => ("Vec<u8>", vector.len()),
            Self {
                dashmap: Some(dashmap),
                ..
            } => (
                "DashMap<u64, Box<[u8]>>",
                dashmap.iter().map(|entry| entry.value().len()).sum(),
            ),
            Self {
                atomics: Some(atomics),
                ..
            } => ("Box<[AtomicU64]>", size_of_val(atomics.as_ref())),
            Self {
                string: Some(string),
                ..
            } => ("String", string.len()),
            Self {
                custom: Some(custom),
                ..
            } => ("Box<CustomTestStruct>", custom.bytes()),
            _ => ("none", 0),
        }
    }
}

static RETAINED_ALLOCATIONS: LazyLock<Mutex<RetainedAllocations>> =
    LazyLock::new(|| Mutex::new(RetainedAllocations::default()));

fn allocate_vector(ctx: &Context, _: Vec<ValkeyString>) -> ValkeyResult {
    // Retains a fixed-size vector without capacity-growth reallocations.
    *RETAINED_ALLOCATIONS
        .lock()
        .expect("usage-tracking allocation mutex should not be poisoned") = RetainedAllocations {
        vector: Some(vec![0u8; ALLOCATION_BYTES]),
        ..Default::default()
    };
    ctx.log_notice("usage_tracking.allocate_vector: retained a 1048576-byte Vec<u8>");

    Ok(ValkeyValue::SimpleStringStatic("OK"))
}

fn allocate_dashmap(ctx: &Context, _: Vec<ValkeyString>) -> ValkeyResult {
    // Retains a pre-sized DashMap with fixed-size values.
    let dashmap = DashMap::with_capacity(DASHMAP_ENTRIES as usize);
    for index in 0..DASHMAP_ENTRIES {
        dashmap.insert(
            index,
            vec![index as u8; ALLOCATION_BYTES / DASHMAP_ENTRIES as usize].into_boxed_slice(),
        );
    }
    *RETAINED_ALLOCATIONS
        .lock()
        .expect("usage-tracking allocation mutex should not be poisoned") = RetainedAllocations {
        dashmap: Some(dashmap),
        ..Default::default()
    };
    ctx.log_notice("usage_tracking.allocate_dashmap: retained a pre-sized DashMap payload");

    Ok(ValkeyValue::SimpleStringStatic("OK"))
}

fn allocate_atomics(ctx: &Context, _: Vec<ValkeyString>) -> ValkeyResult {
    // Retains atomics in boxed heap storage for observable accounting.
    let mut atomics = Vec::with_capacity(ATOMIC_COUNT);
    for index in 0..ATOMIC_COUNT {
        atomics.push(AtomicU64::new(index as u64));
    }
    *RETAINED_ALLOCATIONS
        .lock()
        .expect("usage-tracking allocation mutex should not be poisoned") = RetainedAllocations {
        atomics: Some(atomics.into_boxed_slice()),
        ..Default::default()
    };
    ctx.log_notice("usage_tracking.allocate_atomics: retained a boxed AtomicU64 slice");

    Ok(ValkeyValue::SimpleStringStatic("OK"))
}

fn allocate_string(ctx: &Context, _: Vec<ValkeyString>) -> ValkeyResult {
    // Retains a String that reuses a pre-sized vector allocation.
    let string = String::from_utf8(vec![b's'; ALLOCATION_BYTES])
        .expect("ASCII bytes should always produce a valid UTF-8 string");
    *RETAINED_ALLOCATIONS
        .lock()
        .expect("usage-tracking allocation mutex should not be poisoned") = RetainedAllocations {
        string: Some(string),
        ..Default::default()
    };
    ctx.log_notice("usage_tracking.allocate_string: retained a 1048576-byte String");

    Ok(ValkeyValue::SimpleStringStatic("OK"))
}

fn allocate_custom(ctx: &Context, _: Vec<ValkeyString>) -> ValkeyResult {
    // Retains a boxed user-defined structure with a heap payload.
    let custom = Box::new(CustomTestStruct {
        label: "usage-tracking custom allocation".to_owned(),
        payload: vec![b'c'; ALLOCATION_BYTES],
    });
    debug_assert!(custom.bytes() >= ALLOCATION_BYTES);
    *RETAINED_ALLOCATIONS
        .lock()
        .expect("usage-tracking allocation mutex should not be poisoned") = RetainedAllocations {
        custom: Some(custom),
        ..Default::default()
    };
    ctx.log_notice("usage_tracking.allocate_custom: retained a boxed custom struct");

    Ok(ValkeyValue::SimpleStringStatic("OK"))
}

fn release(ctx: &Context, _: Vec<ValkeyString>) -> ValkeyResult {
    // Drops every retained test allocation.
    *RETAINED_ALLOCATIONS
        .lock()
        .expect("usage-tracking allocation mutex should not be poisoned") =
        RetainedAllocations::default();
    ctx.log_notice("usage_tracking.release: dropped all retained module allocations");

    Ok(ValkeyValue::SimpleStringStatic("OK"))
}

fn snapshot(ctx: &Context, _: Vec<ValkeyString>) -> ValkeyResult {
    // Returns the counters captured before diagnostic logging allocates memory.
    let snapshot = module_stats::snapshot();
    let (retained_kind, retained_bytes) = RETAINED_ALLOCATIONS
        .lock()
        .expect("usage-tracking allocation mutex should not be poisoned")
        .active_kind_and_bytes();
    ctx.log_notice(&format!(
        "usage_tracking.snapshot: current={} peak={} allocs={} frees={} live={} errors={} retained={} retained_bytes={}",
        snapshot.memory_current_bytes,
        snapshot.memory_peak_bytes,
        snapshot.alloc_count,
        snapshot.free_count,
        snapshot.live_allocations,
        snapshot.memory_accounting_errors,
        retained_kind,
        retained_bytes,
    ));

    Ok(ValkeyValue::Array(vec![
        ValkeyValue::Integer(snapshot.memory_current_bytes as i64),
        ValkeyValue::Integer(snapshot.memory_peak_bytes as i64),
        ValkeyValue::Integer(snapshot.alloc_count as i64),
        ValkeyValue::Integer(snapshot.free_count as i64),
        ValkeyValue::Integer(snapshot.live_allocations as i64),
        ValkeyValue::Integer(snapshot.memory_accounting_errors as i64),
    ]))
}

valkey_module! {
    name: "usage_tracking",
    version: 1,
    allocator: (ValkeyAlloc, ValkeyAlloc),
    data_types: [],
    commands: [
        ["usage_tracking.allocate_vector", allocate_vector, "", 0, 0, 0],
        ["usage_tracking.allocate_dashmap", allocate_dashmap, "", 0, 0, 0],
        ["usage_tracking.allocate_atomics", allocate_atomics, "", 0, 0, 0],
        ["usage_tracking.allocate_string", allocate_string, "", 0, 0, 0],
        ["usage_tracking.allocate_custom", allocate_custom, "", 0, 0, 0],
        ["usage_tracking.release", release, "", 0, 0, 0],
        ["usage_tracking.snapshot", snapshot, "", 0, 0, 0],
    ],
}
