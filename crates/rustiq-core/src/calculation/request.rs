use crate::{
    config::{HfConfig, IntegralConfig, MoleculeConfig, Mp2Config},
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
    pub(crate) integrals: IntegralConfig,
    pub(crate) mp2: Option<Mp2Config>,
}

impl CalculationRequest {
    #[must_use]
    pub fn geometry(&self) -> &Geometry {
        &self.geometry
    }

    #[must_use]
    pub fn molecule(&self) -> &MoleculeConfig {
        &self.molecule
    }

    /// Requested portable basis label, or the loaded basis name for direct API use.
    /// This label is not the scientific identity of the resolved basis.
    #[must_use]
    pub fn basis_name(&self) -> &str {
        &self.basis_name
    }

    #[must_use]
    pub fn hf(&self) -> &HfConfig {
        &self.hf
    }

    #[must_use]
    pub fn integrals(&self) -> &IntegralConfig {
        &self.integrals
    }

    #[must_use]
    pub fn mp2(&self) -> Option<&Mp2Config> {
        self.mp2.as_ref()
    }
}
