use std::env;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rustiq_core::bench_support::{BasisStore, EriBenchInput};

// Share application path policy; the scientific core only receives explicit paths.
#[allow(
    dead_code,
    reason = "The benchmark shares the CLI path policy but only uses its basis-store helpers"
)]
#[path = "../src/cli/directories.rs"]
mod cli_directories;

#[derive(Debug, Clone, Copy)]
struct BenchCase {
    name: &'static str,
    basis: &'static str,
    geometry: &'static str,
    heavy: bool,
}

const CASES: &[BenchCase] = &[
    BenchCase {
        name: "h2-sto3g",
        basis: "sto-3g",
        geometry: "samples/h2/molecule.xyz",
        heavy: false,
    },
    BenchCase {
        name: "h2o-sto3g",
        basis: "sto-3g",
        geometry: "samples/h2o/h2o.xyz",
        heavy: false,
    },
    BenchCase {
        name: "oh-sto3g",
        basis: "sto-3g",
        geometry: "samples/oh/oh.xyz",
        heavy: false,
    },
    BenchCase {
        name: "ethanol-sto3g",
        basis: "sto-3g",
        geometry: "samples/ethanol/ethanol.xyz",
        heavy: true,
    },
    BenchCase {
        name: "cholesterol-sto3g",
        basis: "sto-3g",
        geometry: "samples/cholesterol/cholesterol.xyz",
        heavy: true,
    },
];

fn eri_benchmarks(criterion: &mut Criterion) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let basis_store = basis_store();
    let mut group = criterion.benchmark_group("eri");
    for case in selected_cases() {
        // Ten is Criterion's minimum sample size; use it for heavy systems to
        // keep the requested measurement target from multiplying into hours.
        if case.heavy {
            group.sample_size(10);
            group.warm_up_time(Duration::from_secs(1));
            group.measurement_time(Duration::from_secs(2));
        } else {
            group.sample_size(30);
            group.warm_up_time(Duration::from_secs(2));
            group.measurement_time(Duration::from_secs(5));
        }
        let basis_file = basis_store
            .get(case.basis)
            .expect("basis store should be readable")
            .expect("benchmark basis should exist");
        let input = EriBenchInput::load(case.name, manifest_dir.join(case.geometry), basis_file)
            .expect("benchmark geometry and basis inputs should be valid");

        // Keep the existing stage and cache diagnostics outside Criterion's timed iterations.
        let result = input
            .run_once()
            .expect("benchmark input should produce valid Coulomb self-integrals");
        eprintln!(
            "ERI {}: {} basis functions, {} pairs, {} compact integrals, {:?} pair expansion, {:?} Schwarz bounds, {:?} fill, {} cached Coulomb values",
            case.name,
            result.basis_functions,
            result.pair_count,
            result.compact_integrals,
            result.pair_expansions,
            result.schwarz_bounds,
            result.compact_fill,
            result.coulomb_cache_sizes.count
        );

        group.throughput(Throughput::Elements(
            result
                .compact_integrals
                .try_into()
                .expect("compact integral count should fit in u64"),
        ));
        group.bench_function(BenchmarkId::new(case.name, case.basis), |bencher| {
            bencher.iter(|| {
                black_box(
                    input
                        .run_once()
                        .expect("benchmark input should produce valid Coulomb self-integrals"),
                );
            });
        });
    }

    group.finish();
}

fn selected_cases() -> Vec<&'static BenchCase> {
    let include_heavy = env_flag("RUSTIQ_BENCH_HEAVY");
    let filter = env::var("RUSTIQ_BENCH_FILTER").ok();

    CASES
        .iter()
        .filter(|case| include_heavy || !case.heavy)
        .filter(|case| {
            filter
                .as_deref()
                .is_none_or(|filter| case.name.contains(filter) || case.geometry.contains(filter))
        })
        .collect()
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
}

fn basis_store() -> BasisStore {
    if let Ok(path) = env::var("RUSTIQ_BENCH_BASIS_STORE") {
        return BasisStore::new(&PathBuf::from(path));
    }

    let default_store = cli_directories::basis_store();
    if matches!(default_store.get("sto-3g"), Ok(Some(_))) {
        return default_store;
    }

    BasisStore::repository_fixtures()
}

criterion_group!(benches, eri_benchmarks);
criterion_main!(benches);
