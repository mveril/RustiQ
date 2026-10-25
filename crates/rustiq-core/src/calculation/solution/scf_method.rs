/// Describes physical method identity and owned-output representations.
/// This is an output contract, not an executable SCF algorithm.
///
/// Coefficients, energies and occupations have independent representations. The
/// descriptor identifies the physical method; sharing orbital representations
/// does not make two descriptors interchangeable. Implementing this trait does
/// not grant HF operations or provide a calculation engine.
///
/// This contract is open to additional descriptors. Construction of solutions is
/// controlled by the library so that convergence cannot be asserted by callers.
pub trait ScfMethod: std::fmt::Debug {
    /// Coefficient representation, including scalar type and channel layout.
    type Coefficients: std::fmt::Debug;
    /// Orbital energy representation; independent of coefficient topology.
    type OrbitalEnergies: std::fmt::Debug;
    /// Method-specific occupations, independent of coefficient topology.
    /// May represent counts or occupation numbers, including fractional values.
    type Occupations: std::fmt::Debug;
    /// Method-specific summary, including its numerical diagnostics.
    type Summary: std::fmt::Debug;
    /// Retained scientific data specific to this method.
    type RetainedData: std::fmt::Debug;
}
