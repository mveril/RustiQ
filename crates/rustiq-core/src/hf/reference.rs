//! Type-level HF reference contracts, independent of calculation execution.

use nalgebra::{DMatrix, DVector};

use super::spin::Spin;
use crate::config::{HfMethod, ResolvedHfMethod, RestrictedMethod, UnrestrictedMethod};

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
pub trait HfReference: sealed::Sealed + std::fmt::Debug + Clone {
    /// Methods compatible with this orbital layout.
    type Method: Copy + std::fmt::Debug + Into<ResolvedHfMethod> + Into<HfMethod>;
    /// Shape of coefficient-like data: shared or separate per spin.
    type Component<T: std::fmt::Debug + Clone>: std::fmt::Debug + Clone;
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
    type OrbitalEnergies = DVector<f64>;
    type Occupations = usize;
}
impl HfReference for Unrestricted {
    type Method = UnrestrictedMethod;
    type Component<T: std::fmt::Debug + Clone> = Spin<T>;
    type OrbitalEnergies = Spin<DVector<f64>>;
    type Occupations = Spin<usize>;
}

/// MO coefficient matrices in the representation selected by the reference.
pub type MoCoefficients<R> = <R as HfReference>::Component<DMatrix<f64>>;

/// Orbital energies in the representation selected by the reference.
pub type OrbitalEnergies<R> = <R as HfReference>::OrbitalEnergies;

/// Occupations in the representation selected by the reference.
pub type Occupations<R> = <R as HfReference>::Occupations;
