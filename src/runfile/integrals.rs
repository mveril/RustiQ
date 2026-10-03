use toml_spanner::Toml;

use crate::runfile::validated::PositiveFiniteF64;

#[derive(Debug, Toml)]
#[toml(Toml, recoverable)]
pub struct IntegralConfig {
    /// Larger values screen more small ERIs; `0` disables screening.
    #[toml(default = Some(default_schwarz_threshold()))]
    #[toml(with = crate::runfile::validated::optional_positive_finite_f64)]
    pub schwarz_threshold: Option<PositiveFiniteF64>,
}

impl Default for IntegralConfig {
    fn default() -> Self {
        Self {
            schwarz_threshold: Some(default_schwarz_threshold()),
        }
    }
}

fn default_schwarz_threshold() -> PositiveFiniteF64 {
    PositiveFiniteF64::try_new(rustiq_core::config::DEFAULT_ERI_SCHWARZ_THRESHOLD)
        .expect("default ERI Schwarz threshold is positive and finite")
}
