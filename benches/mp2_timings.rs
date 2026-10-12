use std::hint::black_box;
use std::time::Duration;

use bytesize::ByteSize;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rustiq_core::bench_support::Mp2BenchInput;
use rustiq_core::config::MemoryLimit;

fn mp2_benchmarks(criterion: &mut Criterion) {
    let path = std::path::PathBuf::from("target/criterion/mp2-metadata.json");
    if path.exists() {
        std::fs::remove_file(&path).expect("stale MP2 metadata should be removable");
    }
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
    let mut metadata = Vec::new();
    let run_metadata = run_metadata();

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
        assert!(
            occupied >= 2,
            "MP2 benchmark AO dimension must provide at least two occupied orbitals"
        );
        let input = Mp2BenchInput::synthetic(n, occupied);
        // Cap the requested budget at the workspace needed for half of the
        // occupied orbitals. This guarantees that the timed production path
        // executes at least two blocks, even when Auto would fit one block.
        let target_block_size = occupied.div_ceil(2);
        let multiblock_budget = input
            .workspace_for_block_size(target_block_size)
            .expect("the requested MP2 benchmark block should fit in memory");
        let case_budget = budget.as_u64().min(multiblock_budget);
        let dense_energy = input.dense_energy();
        let (blocked_energy, memory) = input
            .blocked_energy(case_budget)
            .expect("the configured synthetic MP2 case fits the benchmark memory budget");
        assert!(
            memory.block_size < occupied,
            "MP2 case must process multiple occupied blocks"
        );
        assert!(
            (dense_energy - blocked_energy).abs() < 1e-12,
            "dense and blocked MP2 energies differ for N={n}"
        );

        group.bench_function(BenchmarkId::new("dense", n), |bencher| {
            bencher.iter(|| black_box(input.dense_energy()));
        });
        group.bench_function(BenchmarkId::new("multiblock", n), |bencher| {
            bencher.iter(|| {
                black_box(
                    input
                        .blocked_energy(case_budget)
                        .expect("the configured synthetic MP2 case fits the memory budget"),
                );
            });
        });

        eprintln!(
            "MP2 N={n}, occupied={occupied}, block={}, blocks={}, budget={}, Rayon threads={}",
            memory.block_size,
            occupied.div_ceil(memory.block_size),
            ByteSize::b(case_budget).display().iec(),
            rayon::current_num_threads()
        );
        metadata.push(serde_json::json!({
            "ao_dimension": n,
            "occupied_orbitals": occupied,
            "workspace_budget_bytes": case_budget,
            "block_size": memory.block_size,
            "block_count": occupied.div_ceil(memory.block_size),
            "workspace_estimate_bytes": memory.workspace_bytes,
            "rayon_threads": rayon::current_num_threads(),
            "dense_energy": dense_energy,
            "blocked_energy": blocked_energy,
            "energy_absolute_difference": (dense_energy - blocked_energy).abs(),
            "criterion_sample_size": if n >= 48 { 10 } else { 30 },
            "criterion_warm_up_seconds": if n >= 48 { 1 } else { 2 },
            "criterion_measurement_seconds": if n >= 48 { 2 } else { 5 },
        }));
    }

    group.finish();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("Criterion metadata directory should be writable");
    }
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&serde_json::json!({"run": run_metadata, "cases": metadata}))
            .expect("metadata should serialize"),
    )
    .expect("Criterion metadata should be writable");
}

fn run_metadata() -> serde_json::Value {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok();
    let commit = output
        .filter(|result| result.status.success())
        .map(|result| String::from_utf8_lossy(&result.stdout).trim().to_owned());
    let compiler = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok();
    let rustc = compiler
        .filter(|result| result.status.success())
        .map(|result| String::from_utf8_lossy(&result.stdout).trim().to_owned());
    let clean = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .filter(|result| result.status.success())
        .map(|result| result.stdout.is_empty());
    let cpu = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|info| {
            info.lines()
                .find(|line| line.starts_with("model name\t:"))
                .map(str::to_owned)
        })
        .and_then(|line| {
            line.split_once(':')
                .map(|(_, value)| value.trim().to_owned())
        });
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs_f64());
    serde_json::json!({"git_commit": commit, "working_tree_clean": clean, "rustc_version": rustc,
        "operating_system": std::env::consts::OS, "architecture": std::env::consts::ARCH,
        "cpu_model": cpu, "benchmark_started_unix_seconds": timestamp,
        "benchmark_profile": "cargo bench default bench profile"})
}

criterion_group!(benches, mp2_benchmarks);
criterion_main!(benches);
