use crate::{
    config::{HfConfig, MoleculeConfig, Mp2Config},
    molecules::geometry::Geometry,
};

/// Frontend-independent, normalized inputs used to prepare a calculation.
///
/// This view keeps requested coordinates and units alongside effective
/// scientific options. It deliberately contains no source text, source spans,
/// frontend syntax, or filesystem paths.
#[derive(Debug, Clone)]
pub struct CalculationRequest {
    pub(crate) geometry: Geometry,
    pub(crate) molecule: MoleculeConfig,
    pub(crate) basis_name: String,
    pub(crate) hf: HfConfig,
    pub(crate) mp2: Option<Mp2Config>,
}

impl CalculationRequest {
    pub fn geometry(&self) -> &Geometry {
        &self.geometry
    }

    pub fn molecule(&self) -> &MoleculeConfig {
        &self.molecule
    }

    /// Canonical basis name associated with the loaded basis data.
    pub fn basis_name(&self) -> &str {
        &self.basis_name
    }

    pub fn hf(&self) -> &HfConfig {
        &self.hf
    }

    pub fn mp2(&self) -> Option<&Mp2Config> {
        self.mp2.as_ref()
    }
}
