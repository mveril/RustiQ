use super::{validated::PositiveFiniteF64, Located};

/// Default Schwarz screening threshold for electron-repulsion integrals.
pub const DEFAULT_ERI_SCHWARZ_THRESHOLD: f64 = 1e-12;

#[derive(Debug, Clone, Copy)]
pub struct IntegralConfig {
    /// Schwarz screening cutoff for ERIs. Larger values discard more small
    /// integrals, reducing ERI computation time at the cost of accuracy;
    /// `None` disables screening.
    pub schwarz_threshold: Located<Option<PositiveFiniteF64>>,
}

impl Default for IntegralConfig {
    fn default() -> Self {
        Self {
            schwarz_threshold: Some(
                PositiveFiniteF64::try_new(DEFAULT_ERI_SCHWARZ_THRESHOLD)
                    .expect("default ERI Schwarz threshold is valid"),
            )
            .into(),
        }
    }
}
