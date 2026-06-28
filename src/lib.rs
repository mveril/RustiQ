#![allow(dead_code, non_snake_case)]

pub mod basis;
pub mod env;

#[cfg(any(feature = "bench-support", test))]
mod eri;
#[cfg(test)]
mod hf;
mod math_utils;
mod molecules;
#[cfg(test)]
mod runfile;
#[cfg(test)]
pub(crate) mod test_utils;

#[cfg(feature = "bench-support")]
pub mod bench_support {
    pub use crate::basis::BasisStore;
    use std::path::Path;
    use std::time::Duration;

    use crate::basis::{gaussian::basis::Basis, BasisFile};
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
        pub coulomb_cache_sizes: crate::eri::CacheSizeStats,
    }

    impl EriBenchInput {
        pub fn load(
            name: impl Into<String>,
            geometry_path: impl AsRef<Path>,
            basis: BasisFile,
        ) -> Self {
            let geometry = Geometry::from_path(geometry_path.as_ref())
                .unwrap_or_else(|err| panic!("failed to read geometry: {err:?}"));
            let basis = Basis::load(&basis, &geometry);

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
                coulomb_cache_sizes: breakdown.coulomb_cache_sizes,
            }
        }
    }
}
