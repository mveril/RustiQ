//! Type-level HF reference contracts, independent of calculation execution.

use nalgebra::{DMatrix, DVector};

use super::spin::Spin;
use crate::config::{ResolvedHfMethod, RestrictedMethod, UnrestrictedMethod};

mod sealed {
    pub trait Sealed {}
}

/// Selects a compatible method and the representations of coefficients, energies, and occupations.
/// These representations are independent: sharing coefficients does not prescribe occupations.
/// Implementations are sealed to the supported HF reference profiles. Each profile fixes
/// all associated representations. A future method requiring a different representation
/// needs its own profile rather than an additional method variant in an incompatible profile.
///
/// ```
/// use rustiq_core::calculation::{HfReference, Restricted, Unrestricted, Spin};
/// let _: <Restricted as HfReference>::Component<usize> = 1;
/// let _: <Restricted as HfReference>::Occupations = 1;
/// let _: <Unrestricted as HfReference>::Occupations = Spin::new(2, 1);
/// fn unrestricted(_: <Unrestricted as HfReference>::Component<usize>) {}
/// fn spin(value: Spin<usize>) { unrestricted(value); }
/// ```
///
/// ```compile_fail
/// use rustiq_core::calculation::{HfReference, Restricted, UnrestrictedMethod};
/// let _: <Restricted as HfReference>::Method = UnrestrictedMethod::Uhf;
/// ```
///
/// ```compile_fail
/// use rustiq_core::calculation::{HfReference, Unrestricted};
/// let _: <Unrestricted as HfReference>::Component<usize> = 1;
/// ```
///
/// A closed-shell reference cannot store unequal alpha and beta counts:
///
/// ```compile_fail
/// use rustiq_core::calculation::{HfReference, Restricted, Spin};
/// let _: <Restricted as HfReference>::Occupations = Spin::new(2, 1);
/// ```
///
/// Orbital energy representations are also reference-specific:
///
/// ```compile_fail
/// use rustiq_core::calculation::{OrbitalEnergies, Restricted, Unrestricted};
/// fn incompatible(energies: OrbitalEnergies<Restricted>) -> OrbitalEnergies<Unrestricted> {
///     energies
/// }
/// ```
pub trait HfReference: sealed::Sealed + std::fmt::Debug + Clone {
    /// Methods compatible with this reference, convertible to a resolved HF method.
    ///
    /// ```
    /// use rustiq_core::calculation::{HfReference, Restricted, RestrictedMethod};
    /// use rustiq_core::config::ResolvedHfMethod;
    /// fn resolved<R: HfReference>(method: R::Method) -> ResolvedHfMethod {
    ///     method.into()
    /// }
    /// assert_eq!(resolved::<Restricted>(RestrictedMethod::Rhf), ResolvedHfMethod::Rhf);
    /// ```
    type Method: Copy + std::fmt::Debug + Into<ResolvedHfMethod>;
    /// Shape of coefficient-like data: shared or separate per spin.
    type Component<T: std::fmt::Debug + Clone>: std::fmt::Debug + Clone;
    /// Create the reference's component layout from one shared value.
    /// Restricted references keep the value; unrestricted references duplicate it.
    fn component_from_shared<T: std::fmt::Debug + Clone>(value: T) -> Self::Component<T>;
    /// Generate one component for restricted references and two for unrestricted references.
    /// Calls are ordered alpha then beta, preserving seeded alpha sequences.
    ///
    /// # Errors
    ///
    /// Returns the first error produced by the generator, without generating any
    /// subsequent components.
    fn try_generate_component<T: std::fmt::Debug + Clone, E>(
        generate: impl FnMut() -> Result<T, E>,
    ) -> Result<Self::Component<T>, E>;
    /// Energy representation, independent of the coefficient layout.
    type OrbitalEnergies: std::fmt::Debug + Clone;
    /// Occupation representation, independent of coefficient and energy layouts.
    type Occupations: std::fmt::Debug + Clone;
}

/// The supported restricted closed-shell reference: shared orbitals and one occupied count.
#[derive(Debug, Clone, Copy)]
pub struct Restricted;
/// Separate alpha and beta spatial orbitals.
#[derive(Debug, Clone, Copy)]
pub struct Unrestricted;
impl sealed::Sealed for Restricted {}
impl sealed::Sealed for Unrestricted {}

impl HfReference for Restricted {
    type Method = RestrictedMethod;
    type Component<T: std::fmt::Debug + Clone> = T;
    fn component_from_shared<T: std::fmt::Debug + Clone>(value: T) -> T {
        value
    }
    fn try_generate_component<T: std::fmt::Debug + Clone, E>(
        mut generate: impl FnMut() -> Result<T, E>,
    ) -> Result<T, E> {
        generate()
    }
    type OrbitalEnergies = DVector<f64>;
    type Occupations = usize;
}
impl HfReference for Unrestricted {
    type Method = UnrestrictedMethod;
    type Component<T: std::fmt::Debug + Clone> = Spin<T>;
    fn component_from_shared<T: std::fmt::Debug + Clone>(value: T) -> Spin<T> {
        Spin::duplicate(value)
    }
    fn try_generate_component<T: std::fmt::Debug + Clone, E>(
        generate: impl FnMut() -> Result<T, E>,
    ) -> Result<Spin<T>, E> {
        Spin::try_generate(generate)
    }
    type OrbitalEnergies = Spin<DVector<f64>>;
    type Occupations = Spin<usize>;
}

/// MO coefficient matrices in the representation selected by the reference.
pub type MoCoefficients<R> = <R as HfReference>::Component<DMatrix<f64>>;

/// Orbital energies in the representation selected by the reference.
pub type OrbitalEnergies<R> = <R as HfReference>::OrbitalEnergies;

/// Occupations in the representation selected by the reference.
pub type Occupations<R> = <R as HfReference>::Occupations;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_component_counts_match_reference_layout() {
        let mut restricted_calls = 0;
        let restricted = Restricted::try_generate_component(|| {
            restricted_calls += 1;
            Ok::<_, ()>(restricted_calls)
        })
        .unwrap();
        assert_eq!(restricted, 1);
        assert_eq!(restricted_calls, 1);

        let mut unrestricted_calls = 0;
        let unrestricted = Unrestricted::try_generate_component(|| {
            unrestricted_calls += 1;
            Ok::<_, ()>(unrestricted_calls)
        })
        .unwrap();
        assert_eq!(*unrestricted.alpha(), 1);
        assert_eq!(*unrestricted.beta(), 2);
        assert_eq!(unrestricted_calls, 2);
    }

    #[test]
    fn shared_components_have_reference_specific_layouts() {
        let restricted = Restricted::component_from_shared(vec![1, 2]);
        let unrestricted = Unrestricted::component_from_shared(vec![1, 2]);
        assert_eq!(restricted, vec![1, 2]);
        assert_eq!(unrestricted.alpha(), &restricted);
        assert_eq!(unrestricted.beta(), &restricted);
    }
}
