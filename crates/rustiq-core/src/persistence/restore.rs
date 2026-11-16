//! Restoration of executable objects from validated scientific artifacts.
use super::{PortableError, RustiQData};

/// Reconstructs an owned object from a RustiQ calculation entry.
///
/// Implementations define their required artifacts and validate them before returning.
/// HF restoration requires converged orbitals and compatible AO ERIs. It never runs
/// SCF or computes missing ERIs, but recomputes one-electron energy components.
/// Persisted HF energies and convergence diagnostics are retained. Reconstructed
/// components use the final orbital density, which can differ from the retained
/// SCF density within the convergence tolerance. Unpersisted execution timings
/// are zero. Restoration trusts the stored convergence record without independently
/// checking orbital canonicality.
///
/// ```no_run
/// use rustiq_core::{persistence::RustiQData, prelude::*};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let data = RustiQData::open("calculation.rustiq")?;
/// let hf: RhfSolution = data.restore()?;
/// let hf_again = RhfSolution::restore_from_rustiq(&data)?;
/// let mp2 = hf.mp2(Mp2Config::default())?;
/// # let _ = (hf_again, mp2);
/// # Ok(())
/// # }
/// ```
///
/// Unconverged solutions cannot be restored through this contract:
///
/// ```compile_fail
/// use rustiq_core::{persistence::RustiQData, prelude::*};
/// fn invalid(data: &RustiQData) {
///     let _ = data.restore::<RhfSolution<Unconverged>>();
/// }
/// ```
pub trait RestoreFromRustiQ: Sized {
    /// Restores this object without modifying the source entry.
    ///
    /// # Errors
    /// Returns an error if required context or artifacts are missing, unsupported,
    /// incompatible, corrupt, or inconsistent with the requested type.
    fn restore_from_rustiq(data: &RustiQData) -> Result<Self, PortableError>;
}

impl RustiQData {
    /// Restores an owned object using its [`RestoreFromRustiQ`] implementation.
    ///
    /// # Errors
    /// Returns the validation or restoration error reported by the requested type.
    pub fn restore<T: RestoreFromRustiQ>(&self) -> Result<T, PortableError> {
        T::restore_from_rustiq(self)
    }
}
