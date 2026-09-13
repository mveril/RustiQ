#![allow(dead_code, non_snake_case)]

//! Reusable domain and scientific implementation used by the CLI and benchmarks.
//!
//! Use [`config`] and [`calculation`] for direct Rust calculations. Runfile parsing,
//! user environment and filesystem policy belong to the application, as do source
//! text for scientific diagnostics and terminal presentation.
//!
//! # Scientific conventions
//!
//! RustiQ uses atomic units throughout the electronic-structure calculation:
//! \(\hbar = m_e = e = 4\pi\varepsilon_0 = 1\). Input coordinates are converted
//! to Bohr before basis construction. AO indices are written as
//! \(\mu, \nu, \lambda, \sigma\), occupied spatial-orbital indices as \(i, j\),
//! and virtual-orbital indices as \(a, b\).
//!
//! The one-electron core Hamiltonian and AO overlap matrix are
//!
//! \[
//! H_{\mu\nu}^{\mathrm{core}} = T_{\mu\nu} + V_{\mu\nu},
//! \qquad S_{\mu\nu} = \braket{\chi_\mu | \chi_\nu}.
//! \]
//!
//! Electron-repulsion integrals use chemists' notation,
//!
//! \[
//! (\mu\nu\mid\lambda\sigma) =
//! \iint \chi_\mu(\mathbf r_1)\chi_\nu(\mathbf r_1)
//! \frac{1}{r_{12}}
//! \chi_\lambda(\mathbf r_2)\chi_\sigma(\mathbf r_2)
//! \, d\mathbf r_1\, d\mathbf r_2.
//! \]
//!
//! Restricted and unrestricted Hartree--Fock use an SCF procedure with symmetric
//! overlap orthogonalization. MP2 is available only from converged, canonical HF
//! orbitals; its reported correlation energy excludes the nuclear-repulsion term.

pub mod basis;
pub mod calculation;
pub mod config;
pub(crate) mod eri;
pub(crate) mod hf;
pub(crate) mod math_utils;
pub mod molecules;
pub(crate) mod mp2;
pub mod prelude;

#[cfg(test)]
pub(crate) mod test_utils;

#[cfg(feature = "bench-support")]
pub mod bench_support {
    pub use crate::basis::BasisStore;
    pub use crate::eri::{CacheSizeStats, EriError};
    use std::path::Path;
    use std::time::Duration;

    use crate::basis::{Basis, BasisFile};
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
        pub coulomb_cache_sizes: CacheSizeStats,
    }

    impl EriBenchInput {
        pub fn load(
            name: impl Into<String>,
            geometry_path: impl AsRef<Path>,
            basis: BasisFile,
        ) -> Result<Self, crate::basis::BasisError> {
            let geometry = Geometry::from_path(geometry_path.as_ref())
                .unwrap_or_else(|err| panic!("failed to read geometry: {err:?}"));
            let basis = Basis::try_load(&basis, &geometry)?;

            Ok(Self {
                name: name.into(),
                basis,
            })
        }

        pub fn run_once(&self) -> Result<EriBenchResult, EriError> {
            self.run_once_with_observer(|_, _| {})
        }

        pub fn run_once_with_observer(
            &self,
            observer: impl FnMut(&'static str, Duration),
        ) -> Result<EriBenchResult, EriError> {
            let (integrals, breakdown) =
                electron_repulsion_ints_timed_with_observer(&self.basis, observer)?;

            Ok(EriBenchResult {
                name: self.name.clone(),
                basis_functions: breakdown.basis_functions,
                pair_count: breakdown.pair_count,
                compact_integrals: integrals.len(),
                pair_expansions: breakdown.pair_expansions,
                schwarz_bounds: breakdown.schwarz_bounds,
                compact_fill: breakdown.compact_fill,
                elapsed: breakdown.total,
                coulomb_cache_sizes: breakdown.coulomb_cache_sizes,
            })
        }
    }
}
