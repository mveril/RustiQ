use std::hint::black_box;
use std::time::Duration;

use bytesize::ByteSize;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rustiq_core::bench_support::Mp2BenchInput;
use rustiq_core::config::MemoryLimit;

fn mp2_benchmarks(criterion: &mut Criterion) {
    let requested = std::env::var("RUSTIQ_MP2_MEMORY").unwrap_or_else(|_| "auto".into());
    let budget = if requested.eq_ignore_ascii_case("auto") {
        MemoryLimit::Auto.resolve()
    } else {
        requested.parse::<ByteSize>().expect("valid memory budget")
    };
    let sizes: Vec<usize> = std::env::var("RUSTIQ_MP2_SIZES")
        .unwrap_or_else(|_| "12,24,48".into())
        .split(',')
        .map(|size| size.parse().expect("integer AO dimension"))
        .collect();

    let mut group = criterion.benchmark_group("mp2");
    for n in sizes {
        // Criterion requires at least ten samples. Large dense MP2 cases are
        // expensive enough that additional samples would dominate bench time.
        if n >= 48 {
            group.sample_size(10);
            group.warm_up_time(Duration::from_secs(1));
            group.measurement_time(Duration::from_secs(2));
        } else {
            group.sample_size(30);
            group.warm_up_time(Duration::from_secs(2));
            group.measurement_time(Duration::from_secs(5));
        }

        let occupied = n / 3;
        let input = Mp2BenchInput::synthetic(n, occupied);
        let dense_energy = input.dense_energy();
        let (blocked_energy, memory) = input
            .blocked_energy(budget.as_u64())
            .expect("the configured synthetic MP2 case fits the benchmark memory budget");
        assert!(
            (dense_energy - blocked_energy).abs() < 1e-12,
            "dense and blocked MP2 energies differ for N={n}"
        );

        group.bench_function(BenchmarkId::new("dense", n), |bencher| {
            bencher.iter(|| black_box(input.dense_energy()));
        });
        group.bench_function(BenchmarkId::new("blocked", n), |bencher| {
            bencher.iter(|| {
                black_box(
                    input
                        .blocked_energy(budget.as_u64())
                        .expect("the configured synthetic MP2 case fits the memory budget"),
                );
            });
        });

        eprintln!(
            "MP2 N={n}, occupied={occupied}, block={}, budget={}, Rayon threads={}",
            memory.block_size,
            budget.display().iec(),
            rayon::current_num_threads()
        );
    }

    group.finish();
}

criterion_group!(benches, mp2_benchmarks);
criterion_main!(benches);
