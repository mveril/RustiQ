use std::env;
use std::io::{self, Write};
use std::path::PathBuf;

use RustiQ::bench_support::{BasisStore, EriBenchInput};

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

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let basis_store = basis_store();
    let cases = selected_cases();

    println!("RustiQ ERI timing bench");
    println!("cases: {}", cases.len());
    println!();

    for case in cases {
        let basis_file = basis_store
            .get(case.basis)
            .unwrap_or_else(|_| panic!("failed to load {} from basis store", case.basis))
            .unwrap_or_else(|| panic!("missing {} in basis store", case.basis));
        let input = EriBenchInput::load(case.name, manifest_dir.join(case.geometry), basis_file);
        println!("case: {}", case.name);
        println!("  basis: {}", case.basis);
        flush_stdout();
        let result = input.run_once_with_observer(|stage, elapsed| {
            println!("  {stage}: {:.3}s", elapsed.as_secs_f64());
            flush_stdout();
        });
        println!("  basis functions: {}", result.basis_functions);
        println!("  basis pairs: {}", result.pair_count);
        println!("  compact integrals: {}", result.compact_integrals);
        print_cache_stats(&result);
        println!("  eri wall: {:.3}s", result.elapsed.as_secs_f64());
        println!();
    }
}

fn print_cache_stats(result: &RustiQ::bench_support::EriBenchResult) {
    let stats = &result.coulomb_cache_sizes;
    println!("  coulomb caches: {}", stats.count);
    println!("  cache len mean: {:.1}", stats.mean_len());
    println!("  cache len max: {}", stats.max_len);
    for capacity in [64, 128, 256, 512, 1024] {
        let covered = stats.count_at_most(capacity);
        let percentage = if stats.count == 0 {
            0.0
        } else {
            covered as f64 * 100.0 / stats.count as f64
        };
        println!("  <= {capacity}: {covered} ({percentage:.1}%)");
    }
}

fn flush_stdout() {
    io::stdout().flush().expect("failed to flush stdout");
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
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn basis_store() -> BasisStore {
    if let Ok(path) = env::var("RUSTIQ_BENCH_BASIS_STORE") {
        return BasisStore::new(&PathBuf::from(path));
    }

    let default_store = BasisStore::default();
    if matches!(default_store.get("sto-3g"), Ok(Some(_))) {
        return default_store;
    }

    BasisStore::repository_fixtures()
}
