use std::{marker::PhantomData, sync::Arc};

use super::{Converged, ScfMethod};

#[derive(Debug)]
struct ScfData<M: ScfMethod> {
    method: M,
    summary: M::Summary,
    coefficients: M::Coefficients,
    energies: M::OrbitalEnergies,
    occupied: M::Occupations,
    retained_data: M::RetainedData,
}

/// Immutable owned SCF output, distinct from the metrics in [`crate::calculation::ScfResult`].
/// Cloning shares all scientific data, without requiring the descriptor or state
/// marker to implement `Clone`. A descriptor determines physical method identity,
/// independent orbital representations, summary and retained data. The generic
/// contract requires no AO ERIs. MP2 is exposed by the converged HF wrapper only.
///
/// ```compile_fail
/// use rustiq_core::prelude::*;
/// fn invalid<M: ScfMethod>(solution: &ScfSolution<M>) {
///     solution.mp2(Mp2Config::default());
/// }
/// ```
///
/// ```compile_fail
/// use rustiq_core::prelude::*;
/// fn invalid<M: ScfMethod>(solution: ScfSolution<M, Unconverged>) -> ScfSolution<M> {
///     solution.into()
/// }
/// ```
/// Sharing all storage representations does not make physical methods interchangeable:
///
/// ```compile_fail
/// use rustiq_core::prelude::*;
/// fn invalid<M, N>(solution: ScfSolution<M>) -> ScfSolution<N>
/// where
///     M: ScfMethod,
///     N: ScfMethod<Coefficients = M::Coefficients,
///         OrbitalEnergies = M::OrbitalEnergies, Occupations = M::Occupations,
///         Summary = M::Summary, RetainedData = M::RetainedData>,
/// {
///     solution.into()
/// }
/// ```
#[derive(Debug)]
pub struct ScfSolution<M: ScfMethod, State = Converged>(Arc<ScfData<M>>, PhantomData<State>);

impl<M: ScfMethod, State> Clone for ScfSolution<M, State> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0), PhantomData)
    }
}

impl<M: ScfMethod, State> ScfSolution<M, State> {
    pub(super) fn new(
        method: M,
        summary: M::Summary,
        coefficients: M::Coefficients,
        energies: M::OrbitalEnergies,
        occupations: M::Occupations,
        retained_data: M::RetainedData,
    ) -> Self {
        Self(
            Arc::new(ScfData {
                method,
                summary,
                coefficients,
                energies,
                occupied: occupations,
                retained_data,
            }),
            PhantomData,
        )
    }

    /// Borrows the physical method descriptor.
    #[must_use]
    pub fn method(&self) -> &M {
        &self.0.method
    }
    /// Borrows the method-specific summary.
    #[must_use]
    pub fn summary(&self) -> &M::Summary {
        &self.0.summary
    }
    /// Borrows the coefficient representation selected by the method.
    #[must_use]
    pub fn mo_coefficients(&self) -> &M::Coefficients {
        &self.0.coefficients
    }
    /// Borrows orbital energies in the method-specific representation.
    #[must_use]
    pub fn orbital_energies(&self) -> &M::OrbitalEnergies {
        &self.0.energies
    }
    /// Borrows the occupation representation selected by the method.
    #[must_use]
    pub fn occupations(&self) -> &M::Occupations {
        &self.0.occupied
    }
    /// Borrows retained scientific data without copying it.
    #[must_use]
    pub fn retained_data(&self) -> &M::RetainedData {
        &self.0.retained_data
    }
}

#[cfg(test)]
mod extensibility_tests {
    use super::*;

    // Deliberately non-HF storage, with shared coefficients but separate fractional spin occupations.
    // No AO ERIs, HF summary, Clone descriptor or shared occupation topology required.
    #[derive(Debug)]
    struct AlternateMethod;
    impl ScfMethod for AlternateMethod {
        type Coefficients = Vec<f64>;
        type OrbitalEnergies = Vec<f64>;
        type Occupations = crate::hf::spin::Spin<f64>;
        type Summary = f64;
        type RetainedData = ();
    }

    #[test]
    fn alternate_output_needs_no_hf_data_and_clones_without_copying() {
        struct State;
        let output: ScfSolution<AlternateMethod, State> = ScfSolution(
            Arc::new(ScfData {
                method: AlternateMethod,
                summary: -0.5,
                coefficients: vec![1.0],
                energies: vec![-0.5],
                occupied: crate::hf::spin::Spin::new(0.75, 0.25),
                retained_data: (),
            }),
            PhantomData,
        );
        let cloned = output.clone();
        assert!(Arc::ptr_eq(&output.0, &cloned.0));
        assert_eq!(*cloned.summary(), -0.5);
        approx::assert_relative_eq!(*cloned.occupations().alpha(), 0.75);
        approx::assert_relative_eq!(*cloned.occupations().beta(), 0.25);
    }
}
