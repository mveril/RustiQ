use bytesize::ByteSize;
use rustiq_core::bench_support::benchmark_mp2;
use rustiq_core::config::MemoryLimit;

fn main() {
    let requested = std::env::var("RUSTIQ_MP2_MEMORY").unwrap_or_else(|_| "auto".into());
    let budget = if requested.eq_ignore_ascii_case("auto") {
        MemoryLimit::Auto.resolve()
    } else {
        requested.parse::<ByteSize>().expect("valid memory budget")
    };
    let sizes: Vec<usize> = std::env::var("RUSTIQ_MP2_SIZES")
        .unwrap_or_else(|_| "12,24,48".into())
        .split(',')
        .map(|s| s.parse().expect("integer AO dimension"))
        .collect();
    println!(
        "MP2 timings: synthetic RHF, budget {}, {} Rayon threads",
        budget.display().iec(),
        rayon::current_num_threads()
    );
    for n in sizes {
        // Warm up kernels before taking the median of three runs.
        let _ = benchmark_mp2(n, n / 3, budget.as_u64()).unwrap();
        let mut runs: Vec<_> = (0..3)
            .map(|_| benchmark_mp2(n, n / 3, budget.as_u64()).unwrap())
            .collect();
        for run in &runs {
            assert!((run.dense_energy - run.blocked_energy).abs() < 1e-12);
        }
        let mut dense: Vec<_> = runs.iter().map(|r| r.dense_elapsed).collect();
        dense.sort();
        runs.sort_by_key(|r| r.blocked_elapsed);
        let run = &runs[1];
        println!("N={n}, O={}, block={}: dense {:?}, blocked {:?}, speedup {:.2}x; dense upper bound {}, blocked payload {}",
            n / 3, run.memory.block_size, dense[1], run.blocked_elapsed,
            dense[1].as_secs_f64() / run.blocked_elapsed.as_secs_f64(),
            ByteSize::b(run.memory.dense_workspace_bytes).display().iec(),
            ByteSize::b(run.memory.workspace_bytes).display().iec());
    }
}
