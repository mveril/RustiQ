#[derive(Debug, Clone)]
#[non_exhaustive]
/// Progress metrics emitted by RustiQ while an SCF calculation runs.
pub struct ScfIteration {
    pub iteration: usize,
    pub electronic_energy: f64,
    pub delta_energy: f64,
    pub residual_norm: f64,
}
