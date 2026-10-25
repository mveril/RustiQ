/// The library has established convergence according to the method's criteria.
/// For the supported HF implementations, this also includes final canonicalization.
#[derive(Debug, Clone, Copy)]
pub struct Converged;

/// The library has established that the method did not reach convergence.
/// The supported HF implementations use this state when the iteration budget is exhausted.
#[derive(Debug, Clone, Copy)]
pub struct Unconverged;
