#![allow(dead_code, non_snake_case)]

#[cfg(any(feature = "bench-support", test))]
mod basis {
    pub(crate) mod basisfile;
    pub(crate) mod function_type;
    pub(crate) mod gaussian;
    mod utils;
}
#[cfg(any(feature = "bench-support", test))]
mod eri;
#[cfg(test)]
mod hf;
#[cfg(any(feature = "bench-support", test))]
mod math_utils;
#[cfg(any(feature = "bench-support", test))]
mod molecules;
#[cfg(test)]
mod runfile;
#[cfg(test)]
pub(crate) mod test_utils;

#[cfg(feature = "bench-support")]
pub mod bench_support {
    use std::path::Path;
    use std::time::Duration;

    use crate::basis::{basisfile::BasisFile, gaussian::basis::Basis};
    use crate::eri::electron_repulsion_ints_timed_with_observer;
    use crate::molecules::geometry::Geometry;

    pub struct EriBenchInput {
        name: String,
        basis: Basis,
    }

    pub struct EriBenchResult {
        pub name: String,
        pub basis_functions: usize,
        pub pair_count: usize,
        pub compact_integrals: usize,
        pub pair_expansions: Duration,
        pub schwarz_bounds: Duration,
        pub compact_fill: Duration,
        pub elapsed: Duration,
    }

    impl EriBenchInput {
        pub fn load_sto3g(
            name: impl Into<String>,
            geometry_path: impl AsRef<Path>,
            basis_path: impl AsRef<Path>,
        ) -> Self {
            let geometry = Geometry::from_path(geometry_path.as_ref())
                .unwrap_or_else(|err| panic!("failed to read geometry: {err:?}"));
            let basis_source = std::fs::read_to_string(basis_path.as_ref())
                .unwrap_or_else(|err| panic!("failed to read basis file: {err:?}"));
            let basis_file: BasisFile = serde_json::from_str(&basis_source)
                .unwrap_or_else(|err| panic!("failed to parse basis file: {err:?}"));
            let basis = Basis::load(&basis_file, &geometry);

            Self {
                name: name.into(),
                basis,
            }
        }

        pub fn run_once(&self) -> EriBenchResult {
            self.run_once_with_observer(|_, _| {})
        }

        pub fn run_once_with_observer(
            &self,
            observer: impl FnMut(&'static str, Duration),
        ) -> EriBenchResult {
            let (integrals, breakdown) =
                electron_repulsion_ints_timed_with_observer(&self.basis, observer);

            EriBenchResult {
                name: self.name.clone(),
                basis_functions: breakdown.basis_functions,
                pair_count: breakdown.pair_count,
                compact_integrals: integrals.len(),
                pair_expansions: breakdown.pair_expansions,
                schwarz_bounds: breakdown.schwarz_bounds,
                compact_fill: breakdown.compact_fill,
                elapsed: breakdown.total,
            }
        }
    }
}
