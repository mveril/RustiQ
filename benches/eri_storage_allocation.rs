use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rustiq_core::bench_support::{build_eri_storage_in_place, build_eri_storage_vec_then_arc};

fn eri_storage_allocation(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("eri_storage_allocation");

    for basis_functions in [32, 48, 64] {
        group.bench_with_input(
            BenchmarkId::new("vec_then_arc", basis_functions),
            &basis_functions,
            |bencher, &size| bencher.iter(|| black_box(build_eri_storage_vec_then_arc(size))),
        );
        group.bench_with_input(
            BenchmarkId::new("in_place", basis_functions),
            &basis_functions,
            |bencher, &size| bencher.iter(|| black_box(build_eri_storage_in_place(size))),
        );
    }

    group.finish();
}

criterion_group!(benches, eri_storage_allocation);
criterion_main!(benches);
