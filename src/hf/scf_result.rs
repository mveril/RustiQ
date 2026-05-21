#[derive(Debug, Clone)]
pub struct ScfResult {
    pub converged: bool,
    pub iterations: usize,
    pub electronic_energy: f64,
    pub nuclear_repulsion_energy: f64,
    pub total_energy: f64,
    pub delta_energy: f64,
    pub residual_norm: f64,
}
