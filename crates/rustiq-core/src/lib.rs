// Numerical accuracy and terminal output isolation are library-specific requirements.
#![deny(clippy::imprecise_flops, clippy::print_stdout, clippy::print_stderr)]
#![allow(
    dead_code,
    non_snake_case,
    reason = "Retained scientific helpers and representations support tests, benchmarks, or future internal use; Symbols follow established matrix and Gaussian integral notation"
)]
#![allow(
    clippy::doc_markdown,
    reason = "The crate introduction uses MathJax equations; child modules re-enable Markdown linting"
)]

//! Reusable domain and scientific implementation used by the CLI and benchmarks.
//!
//! Use [`config`] and [`calculation`] for direct Rust calculations. Runfile parsing,
//! user environment and filesystem policy belong to the application, as do source
//! text for scientific diagnostics and terminal presentation. Machine-resource
//! discovery used by scientific execution is isolated from scientific configuration.
//!
//! # Scientific conventions
//!
//! RustiQ uses atomic units throughout the electronic-structure calculation:
//! $\hbar = m_e = e = 4\pi\varepsilon_0 = 1$. Input coordinates are converted
//! to Bohr before basis construction. AO indices are written as
//! $\mu, \nu, \lambda, \sigma$, occupied spatial-orbital indices as $i, j$,
//! and virtual-orbital indices as $a, b$.
//!
//! The one-electron core Hamiltonian and AO overlap matrix are
//!
//! $$
//! H_{\mu\nu}^{\mathrm{core}} = T_{\mu\nu} + V_{\mu\nu},
//! \qquad S_{\mu\nu} = \braket{\chi_\mu | \chi_\nu}.
//! $$
//!
//! Electron-repulsion integrals use chemists' notation,
//!
//! $$
//! (\mu\nu\mid\lambda\sigma) =
//! \iint \chi_\mu(\mathbf r_1)\chi_\nu(\mathbf r_1)
//! \frac{1}{r_{12}}
//! \chi_\lambda(\mathbf r_2)\chi_\sigma(\mathbf r_2)
//! \\, d\mathbf r_1\\, d\mathbf r_2.
//! $$
//!
//! Restricted and unrestricted Hartree--Fock use an SCF procedure with overlap
//! orthogonalization; near-linear dependencies may be discarded. See
//! [`calculation`] for the RHF/UHF equations, orthogonalization convention, and
//! MP2 correction. MP2 is available only from converged, canonical HF orbitals.

#[deny(clippy::doc_markdown)]
pub mod basis;
#[deny(clippy::doc_markdown)]
pub mod calculation;
#[deny(clippy::doc_markdown)]
pub mod config;
#[deny(clippy::doc_markdown)]
pub(crate) mod eri;
#[deny(clippy::doc_markdown)]
pub(crate) mod hf;
#[deny(clippy::doc_markdown)]
pub(crate) mod math_utils;
#[deny(clippy::doc_markdown)]
pub mod molecules;
#[deny(clippy::doc_markdown)]
pub(crate) mod mp2;
#[deny(clippy::doc_markdown)]
pub mod persistence;
#[deny(clippy::doc_markdown)]
pub mod prelude;
#[deny(clippy::doc_markdown)]
pub(crate) mod resources;

#[cfg(test)]
#[deny(clippy::doc_markdown)]
pub(crate) mod test_utils;

#[cfg(feature = "bench-support")]
#[deny(clippy::doc_markdown)]
pub mod bench_support {
    pub use crate::basis::BasisStore;
    pub use crate::eri::{CacheSizeStats, EriError};
    pub use crate::mp2::{benchmark_mp2, Mp2BenchResult};
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
        /// Loads a geometry and basis for repeated ERI benchmarks.
        ///
        /// # Errors
        ///
        /// Returns an error if the basis data is invalid or unsupported.
        ///
        /// # Panics
        ///
        /// Panics if the benchmark geometry cannot be read or parsed.
        #[allow(
            clippy::needless_pass_by_value,
            reason = "Preserve the public benchmark loader ownership contract"
        )]
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

        /// Computes ERIs once and returns the timing breakdown.
        ///
        /// # Errors
        ///
        /// Returns an error if ERI allocation, size calculations, or numerical evaluation fails.
        pub fn run_once(&self) -> Result<EriBenchResult, EriError> {
            self.run_once_with_observer(|_, _| {})
        }

        /// Computes ERIs once while reporting timings for each stage.
        ///
        /// # Errors
        ///
        /// Returns an error if ERI allocation, size calculations, or numerical evaluation fails.
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
