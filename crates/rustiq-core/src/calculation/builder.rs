use crate::{
    basis::{gaussian::basis::Basis, BasisFile},
    config::{HfConfig, MoleculeConfig, Mp2Config},
    molecules::{geometry::Geometry, units::Units},
};
use std::time::Instant;

use super::{
    CalculationError, CalculationExecution, CalculationObserver, CalculationResult,
    NoopCalculationObserver, PreparedCalculation,
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
    hf: Option<HfConfig>,
    mp2: Option<Mp2Config>,
}

impl<'a> CalculationBuilder<'a> {
    pub fn new(geometry: &'a Geometry, basis_file: &'a BasisFile) -> Self {
        Self {
            geometry,
            basis_file,
            molecule_config: MoleculeConfig::default(),
            hf: Some(HfConfig::default()),
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
    pub fn get_hf(&self) -> Option<&HfConfig> {
        self.hf.as_ref()
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

    /// Configure HF, or pass `None` to only prepare the molecule and basis.
    pub fn hf(&mut self, config: impl Into<Option<HfConfig>>) -> &mut Self {
        self.hf = config.into();
        self
    }

    #[must_use]
    pub fn with_hf(mut self, config: impl Into<Option<HfConfig>>) -> Self {
        self.hf(config);
        self
    }

    /// Configure MP2, or pass `None` to disable it. MP2 requires HF.
    pub fn mp2(&mut self, config: impl Into<Option<Mp2Config>>) -> &mut Self {
        self.mp2 = config.into();
        self
    }

    #[must_use]
    pub fn with_mp2(mut self, config: impl Into<Option<Mp2Config>>) -> Self {
        self.mp2(config);
        self
    }

    /// Prepare reusable scientific inputs without executing HF or MP2.
    pub fn prepare(&self) -> Result<PreparedCalculation, CalculationError> {
        self.prepare_with_observer(&mut NoopCalculationObserver)
    }

    pub fn prepare_with_observer(
        &self,
        observer: &mut impl CalculationObserver,
    ) -> Result<PreparedCalculation, CalculationError> {
        if self.mp2.is_some() && self.hf.is_none() {
            return Err(CalculationError::Mp2RequiresHf);
        }
        let mut molecule = self.molecule_config.build(self.geometry.clone())?;
        molecule.convert_to(Units::Bohr);
        let hf = self
            .hf
            .as_ref()
            .map(|config| {
                config
                    .resolve_method(&molecule)
                    .map(|method| (config.clone(), method))
            })
            .transpose()?;
        observer.on_basis_start();
        let start = Instant::now();
        let basis = Basis::try_load(self.basis_file, &molecule)?;
        observer.on_basis_ready(&basis, start.elapsed());
        Ok(PreparedCalculation {
            molecule,
            basis,
            hf,
            mp2: self.mp2,
        })
    }
}

impl CalculationExecution for CalculationBuilder<'_> {
    fn execute_with_observer(
        &self,
        observer: &mut impl CalculationObserver,
    ) -> Result<CalculationResult, CalculationError> {
        self.prepare_with_observer(observer)?
            .execute_with_observer(observer)
    }
}
