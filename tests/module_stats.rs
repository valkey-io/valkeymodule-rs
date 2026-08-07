use valkey_module::module_stats::ModuleStats;

#[test]
fn snapshot_tracks_current_peak_counts_and_live_allocations() {
    let stats = ModuleStats::new();

    stats.add_alloc(64);
    stats.add_alloc(32);
    stats.add_free(64);

    let snapshot = stats.snapshot();

    assert_eq!(snapshot.memory_current_bytes, 32);
    assert_eq!(snapshot.memory_peak_bytes, 96);
    assert_eq!(snapshot.alloc_count, 2);
    assert_eq!(snapshot.free_count, 1);
    assert_eq!(snapshot.live_allocations, 1);
    assert_eq!(snapshot.memory_accounting_errors, 0);
}

#[test]
fn peak_is_retained_after_memory_is_freed() {
    let stats = ModuleStats::new();

    stats.add_alloc(100);
    stats.add_free(60);

    let snapshot = stats.snapshot();

    assert_eq!(snapshot.memory_current_bytes, 40);
    assert_eq!(snapshot.memory_peak_bytes, 100);
    assert_eq!(snapshot.alloc_count, 1);
    assert_eq!(snapshot.free_count, 1);
    assert_eq!(snapshot.live_allocations, 0);
    assert_eq!(snapshot.memory_accounting_errors, 0);
}

#[test]
fn freeing_more_than_current_memory_saturates_and_records_error() {
    let stats = ModuleStats::new();

    stats.add_alloc(16);
    stats.add_free(24);

    let snapshot = stats.snapshot();

    assert_eq!(snapshot.memory_current_bytes, 0);
    assert_eq!(snapshot.memory_peak_bytes, 16);
    assert_eq!(snapshot.alloc_count, 1);
    assert_eq!(snapshot.free_count, 1);
    assert_eq!(snapshot.live_allocations, 0);
    assert_eq!(snapshot.memory_accounting_errors, 1);
}

#[test]
fn allocation_overflow_saturates_and_records_error() {
    let stats = ModuleStats::new();

    stats.add_alloc(u64::MAX);
    stats.add_alloc(1);

    let snapshot = stats.snapshot();

    assert_eq!(snapshot.memory_current_bytes, u64::MAX);
    assert_eq!(snapshot.memory_peak_bytes, u64::MAX);
    assert_eq!(snapshot.alloc_count, 2);
    assert_eq!(snapshot.free_count, 0);
    assert_eq!(snapshot.live_allocations, 2);
    assert_eq!(snapshot.memory_accounting_errors, 1);
}
