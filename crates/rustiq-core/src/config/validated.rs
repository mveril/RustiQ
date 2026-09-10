//! Validated scalar parameters for scientific calculations.
use nutype::nutype;

#[nutype(
    validate(finite, greater = 0.0),
    derive(Debug, Clone, Copy, PartialEq, PartialOrd, TryFrom, Into)
)]
pub struct PositiveFiniteF64(f64);

#[nutype(
    validate(finite, greater_or_equal = 0.0),
    derive(Debug, Clone, Copy, PartialEq, PartialOrd, TryFrom, Into)
)]
pub struct NonNegativeFiniteF64(f64);

#[nutype(
    validate(greater = 1),
    derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFrom, Into)
)]
pub struct DiisSize(usize);
