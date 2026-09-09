use crate::{
    basis::{gaussian::basis::Basis, BasisFile},
    config::{HfConfig, MoleculeConfig, Mp2Config},
    molecules::{geometry::Geometry, units::Units},
};
use std::time::Instant;

use super::{
    CalculationError, CalculationEvent, CalculationExecution, CalculationResult,
    PreparedCalculation,
};

/// Configure a calculation from explicitly loaded inputs.
///
/// Setters return `&mut Self`; `with_*` variants consume and return the builder.
/// Preparation validates the molecular state and HF/MP2 combination, converts
/// coordinates to Bohr, and builds the basis at those coordinates. Neither the
/// input geometry nor the basis file is modified. Defaults describe a neutral
/// singlet in Bohr with automatic HF selection and no MP2.
///
/// ```no_run
/// use rustiq_core::{
///     basis::BasisFile,
///     calculation::{CalculationBuilder, CalculationExecution},
///     config::{HfConfig, MoleculeConfig, Mp2Config},
///     molecules::{geometry::Geometry, units::Units},
/// };
/// # fn example(geometry: &Geometry, basis_file: &BasisFile) -> Result<(), Box<dyn std::error::Error>> {
/// let calculation = CalculationBuilder::new(geometry, basis_file)
///     .with_molecule_config(MoleculeConfig { units: Units::Angstrom, ..Default::default() })
///     .with_hf(HfConfig { diis: true, ..Default::default() })
///     .with_mp2(Mp2Config::default());
/// let prepared = calculation.prepare()?;
/// let result = prepared.execute()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct CalculationBuilder<'a> {
    geometry: &'a Geometry,
    basis_file: &'a BasisFile,
    molecule_config: MoleculeConfig,
    hf: HfConfig,
    mp2: Option<Mp2Config>,
}

impl<'a> CalculationBuilder<'a> {
    pub fn new(geometry: &'a Geometry, basis_file: &'a BasisFile) -> Self {
        Self {
            geometry,
            basis_file,
            molecule_config: MoleculeConfig::default(),
            hf: HfConfig::default(),
            mp2: None,
        }
    }

    pub fn get_geometry(&self) -> &Geometry {
        self.geometry
    }
    pub fn get_basis_file(&self) -> &BasisFile {
        self.basis_file
    }
    pub fn get_molecule_config(&self) -> &MoleculeConfig {
        &self.molecule_config
    }
    pub fn get_hf(&self) -> &HfConfig {
        &self.hf
    }
    pub fn get_mp2(&self) -> Option<&Mp2Config> {
        self.mp2.as_ref()
    }

    pub fn molecule_config(&mut self, config: MoleculeConfig) -> &mut Self {
        self.molecule_config = config;
        self
    }

    #[must_use]
    pub fn with_molecule_config(mut self, config: MoleculeConfig) -> Self {
        self.molecule_config(config);
        self
    }

    /// Configure the mandatory HF stage.
    pub fn hf(&mut self, config: HfConfig) -> &mut Self {
        self.hf = config;
        self
    }

    #[must_use]
    pub fn with_hf(mut self, config: HfConfig) -> Self {
        self.hf(config);
        self
    }

    /// Configure MP2, or pass `None` to disable it.
    pub fn mp2(&mut self, config: impl Into<Option<Mp2Config>>) -> &mut Self {
        self.mp2 = config.into();
        self
    }

    /// Disable MP2 by clearing its configuration.
    pub fn clear_mp2(&mut self) -> &mut Self {
        self.mp2 = None;
        self
    }

    #[must_use]
    pub fn with_mp2(mut self, config: impl Into<Option<Mp2Config>>) -> Self {
        self.mp2(config);
        self
    }
}

impl<'a> CalculationBuilder<'a> {
    /// Prepare reusable scientific inputs without executing HF or MP2.
    pub fn prepare(&self) -> Result<PreparedCalculation, CalculationError> {
        self.prepare_with_events(|_| {})
    }

    pub fn prepare_with_events(
        &self,
        mut events: impl FnMut(CalculationEvent<'_>),
    ) -> Result<PreparedCalculation, CalculationError> {
        let mut molecule = self.molecule_config.build(self.geometry.clone())?;
        molecule.convert_to(Units::Bohr);
        let hf = (self.hf.clone(), self.hf.resolve_method(&molecule)?);
        events(CalculationEvent::BasisStarted);
        let start = Instant::now();
        let basis = Basis::try_load(self.basis_file, &molecule)?;
        events(CalculationEvent::BasisReady {
            basis: &basis,
            elapsed: start.elapsed(),
        });
        Ok(PreparedCalculation {
            molecule,
            basis,
            hf,
            mp2: self.mp2,
        })
    }
}

impl CalculationExecution for CalculationBuilder<'_> {
    fn execute_with_events(
        &self,
        mut events: impl FnMut(CalculationEvent<'_>),
    ) -> Result<CalculationResult, CalculationError> {
        self.prepare_with_events(&mut events)?
            .execute_with_events(events)
    }
}
