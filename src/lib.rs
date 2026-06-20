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
    use std::time::{Duration, Instant};

    use crate::basis::{basisfile::BasisFile, gaussian::basis::Basis};
    use crate::eri::electron_repulsion_ints;
    use crate::molecules::geometry::Geometry;

    pub struct EriBenchInput {
        name: String,
        basis: Basis,
    }

    pub struct EriBenchResult {
        pub name: String,
        pub basis_functions: usize,
        pub compact_integrals: usize,
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
            let start = Instant::now();
            let integrals = electron_repulsion_ints(&self.basis);
            let elapsed = start.elapsed();

            EriBenchResult {
                name: self.name.clone(),
                basis_functions: self.basis.nbasis(),
                compact_integrals: integrals.len(),
                elapsed,
            }
        }
    }
}
