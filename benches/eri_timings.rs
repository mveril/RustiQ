use std::env;
use std::path::PathBuf;

use RustiQ::bench_support::EriBenchInput;

#[derive(Debug, Clone, Copy)]
struct BenchCase {
    name: &'static str,
    geometry: &'static str,
    heavy: bool,
}

const CASES: &[BenchCase] = &[
    BenchCase {
        name: "h2-sto3g",
        geometry: "samples/h2/molecule.xyz",
        heavy: false,
    },
    BenchCase {
        name: "h2o-sto3g",
        geometry: "samples/h2o/h2o.xyz",
        heavy: false,
    },
    BenchCase {
        name: "oh-sto3g",
        geometry: "samples/oh/oh.xyz",
        heavy: false,
    },
    BenchCase {
        name: "ethanol-sto3g",
        geometry: "samples/ethanol/ethanol.xyz",
        heavy: true,
    },
    BenchCase {
        name: "cholesterol-sto3g",
        geometry: "samples/cholesterol/cholesterol.xyz",
        heavy: true,
    },
];

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let basis_path = basis_path(&manifest_dir);
    let cases = selected_cases();

    println!("RustiQ ERI timing bench");
    println!("basis: {}", basis_path.display());
    println!("cases: {}", cases.len());
    println!();

    for case in cases {
        let input =
            EriBenchInput::load_sto3g(case.name, manifest_dir.join(case.geometry), &basis_path);
        let result = input.run_once();
        println!("case: {}", result.name);
        println!("  basis functions: {}", result.basis_functions);
        println!("  compact integrals: {}", result.compact_integrals);
        println!("  eri wall: {:.3}s", result.elapsed.as_secs_f64());
        println!();
    }
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

fn basis_path(manifest_dir: &PathBuf) -> PathBuf {
    if let Ok(path) = env::var("RUSTIQ_BENCH_BASIS") {
        return PathBuf::from(path);
    }

    let local_cache = env::var("HOME")
        .map(PathBuf::from)
        .map(|home| {
            home.join(".local")
                .join("share")
                .join("RustiQ")
                .join("basis_sets")
                .join("sto-3g.json")
        })
        .ok();
    if let Some(path) = local_cache.filter(|path| path.exists()) {
        return path;
    }

    manifest_dir.join("tests").join("data").join("sto-3g.json")
}
