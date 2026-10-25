pub use distribution_config::DistributionCreationError;
pub mod distribution_config;
pub use distribution_config::DistributionConfig;
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::config::random_config::distribution_config::SelectedSampleIter;

/// Distribution and optional seed used when generating an initial guess.
///
/// Construct this configuration with [`RandomConfig::new`] and set `seed` when
/// deterministic sampling is required. Without a seed, each sampling operation
/// initializes its generator from system randomness.
///
/// ```
/// use rustiq_core::config::{DistributionConfig, RandomConfig, UniformDistributionConfig};
/// let random = RandomConfig::new(DistributionConfig::Uniform {
///     config: UniformDistributionConfig { min: -1.0, max: 1.0 },
/// });
/// assert_eq!(random.seed, None);
/// ```
///
/// A struct literal would prevent RustiQ from adding compatible options:
///
/// ```compile_fail
/// use rustiq_core::config::{DistributionConfig, RandomConfig, UniformDistributionConfig};
/// let random = RandomConfig {
///     distribution: DistributionConfig::Uniform {
///         config: UniformDistributionConfig { min: -1.0, max: 1.0 },
///     },
///     seed: None,
/// };
/// ```
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct RandomConfig {
    pub distribution: DistributionConfig,
    pub seed: Option<u64>,
}

impl RandomConfig {
    /// Creates a random configuration with no explicit seed.
    ///
    /// Without a seed, each sampling operation initializes its generator from
    /// system randomness. Set [`RandomConfig::seed`] when reproducible sampling
    /// is required.
    #[must_use]
    pub const fn new(distribution: DistributionConfig) -> Self {
        Self {
            distribution,
            seed: None,
        }
    }

    pub(crate) fn sample_iter(
        &self,
    ) -> Result<SelectedSampleIter, distribution_config::DistributionCreationError> {
        let rng = if let Some(seed) = self.seed {
            StdRng::seed_from_u64(seed)
        } else {
            StdRng::from_rng(&mut rand::rng())
        };
        self.distribution.sample_iter(rng)
    }
}
