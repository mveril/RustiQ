#![allow(
    unsafe_code,
    reason = "The benchmark needs a counting global allocator to measure peak live heap bytes"
)]

use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicUsize, Ordering},
};

use rustiq_core::bench_support::{build_eri_storage_in_place, build_eri_storage_vec_then_arc};

struct TrackingAllocator;

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            record_allocation(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            record_allocation(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            if new_size >= layout.size() {
                record_allocation(new_size - layout.size());
            } else {
                LIVE_BYTES.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        new_ptr
    }
}

fn record_allocation(size: usize) {
    let live = LIVE_BYTES.fetch_add(size, Ordering::Relaxed) + size;
    PEAK_BYTES.fetch_max(live, Ordering::Relaxed);
}

fn measure_peak_delta(build: impl FnOnce() -> rustiq_core::bench_support::CompactEri) -> usize {
    let baseline = LIVE_BYTES.load(Ordering::Relaxed);
    PEAK_BYTES.store(baseline, Ordering::Relaxed);

    let storage = black_box(build());
    let peak = PEAK_BYTES.load(Ordering::Relaxed);
    let peak_delta = peak.saturating_sub(baseline);
    drop(storage);

    assert_eq!(LIVE_BYTES.load(Ordering::Relaxed), baseline);
    peak_delta
}

fn compact_eri_payload_bytes(basis_functions: usize) -> usize {
    let pair_count = basis_functions * (basis_functions + 1) / 2;
    let eri_count = pair_count * (pair_count + 1) / 2;
    eri_count * std::mem::size_of::<f64>()
}

fn main() {
    println!("basis | storage payload | vec-then-arc peak | in-place peak");
    for basis_functions in [32, 48, 64] {
        let payload_bytes = compact_eri_payload_bytes(basis_functions);
        let vec_then_arc_peak =
            measure_peak_delta(|| build_eri_storage_vec_then_arc(basis_functions));
        let in_place_peak = measure_peak_delta(|| build_eri_storage_in_place(basis_functions));

        println!(
            "{basis_functions:>5} | {payload_bytes:>14} B | {vec_then_arc_peak:>17} B | {in_place_peak:>13} B"
        );
    }
}
