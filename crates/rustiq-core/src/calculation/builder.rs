use crate::{
    basis::{Basis, BasisFile},
    config::{HfConfig, IntegralConfig, MoleculeConfig, Mp2Config},
    molecules::{geometry::Geometry, units::Units},
    persistence::EriCache,
};
use std::time::Instant;

use super::{
    CalculationError, CalculationEvent, CalculationExecution, CalculationExecutionError,
    CalculationRequest, CalculationResult, PreparedCalculation,
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
///     .with_hf(HfConfig {
///         diis: rustiq_core::config::DiisConfig {
///             enabled: true,
///             ..Default::default()
///         },
///         ..Default::default()
///     })
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
    basis_label: Option<String>,
    molecule_config: MoleculeConfig,
    hf: HfConfig,
    integrals: IntegralConfig,
    mp2: Option<Mp2Config>,
    eri_cache: Option<EriCache>,
}

impl<'a> CalculationBuilder<'a> {
    pub fn new(geometry: &'a Geometry, basis_file: &'a BasisFile) -> Self {
        Self {
            geometry,
            basis_file,
            basis_label: None,
            molecule_config: MoleculeConfig::default(),
            hf: HfConfig::default(),
            integrals: IntegralConfig::default(),
            mp2: None,
            eri_cache: None,
        }
    }

    pub fn get_geometry(&self) -> &Geometry {
        self.geometry
    }
    pub fn get_basis_file(&self) -> &BasisFile {
        self.basis_file
    }

    /// Set a portable requested basis label; the loaded basis remains authoritative.
    pub fn basis_label(&mut self, label: impl Into<String>) -> &mut Self {
        self.basis_label = Some(label.into());
        self
    }

    #[must_use]
    pub fn with_basis_label(mut self, label: impl Into<String>) -> Self {
        self.basis_label(label);
        self
    }
    pub fn get_molecule_config(&self) -> &MoleculeConfig {
        &self.molecule_config
    }
    pub fn get_hf(&self) -> &HfConfig {
        &self.hf
    }
    pub fn get_integrals(&self) -> &IntegralConfig {
        &self.integrals
    }

    pub fn get_mp2(&self) -> Option<&Mp2Config> {
        self.mp2.as_ref()
    }
    pub fn get_eri_cache(&self) -> Option<&EriCache> {
        self.eri_cache.as_ref()
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

    pub fn integrals(&mut self, config: IntegralConfig) -> &mut Self {
        self.integrals = config;
        self
    }

    #[must_use]
    pub fn with_integrals(mut self, config: IntegralConfig) -> Self {
        self.integrals(config);
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

    /// Use an explicitly located cache for deterministic AO ERIs.
    pub fn eri_cache(&mut self, cache: impl Into<Option<EriCache>>) -> &mut Self {
        self.eri_cache = cache.into();
        self
    }

    #[must_use]
    pub fn with_eri_cache(mut self, cache: impl Into<Option<EriCache>>) -> Self {
        self.eri_cache(cache);
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
        let request = self.normalized_request();
        let mut requested_geometry = self.geometry.clone();
        requested_geometry.comment.clear();
        let mut molecule = self.molecule_config.build(requested_geometry.clone())?;
        molecule.convert_to(Units::Bohr);
        let method = self.hf.resolve_method(&molecule)?;
        let mut execution_hf = self.hf.clone();
        resolve_random_seeds(&mut execution_hf);
        let hf = (execution_hf, method);
        events(CalculationEvent::BasisStarted);
        let start = Instant::now();
        let basis = Basis::try_load(self.basis_file, &molecule)?;
        events(CalculationEvent::BasisReady {
            basis: &basis,
            elapsed: start.elapsed(),
        });
        Ok(PreparedCalculation {
            request,
            molecule,
            basis,
            basis_name: self.basis_file.name().to_owned(),
            hf,
            integrals: self.integrals,
            mp2: self.mp2,
            eri_cache: self.eri_cache.clone(),
        })
    }

    fn normalized_request(&self) -> CalculationRequest {
        let mut geometry = self.geometry.clone();
        geometry.comment.clear();
        CalculationRequest {
            geometry,
            molecule: MoleculeConfig {
                units: self.molecule_config.units,
                charge: self.molecule_config.charge.value.into(),
                multiplicity: self.molecule_config.multiplicity.value.into(),
            },
            basis_name: self
                .basis_label
                .clone()
                .unwrap_or_else(|| self.basis_file.name().to_owned()),
            hf: normalized_hf_config(&self.hf),
            integrals: normalized_integral_config(&self.integrals),
            mp2: self.mp2.map(|config| Mp2Config {
                frozen_orbitals: config.frozen_orbitals.value.into(),
                memory_limit: config.memory_limit.value.into(),
            }),
        }
    }
}

pub(super) fn normalized_integral_config(config: &IntegralConfig) -> IntegralConfig {
    IntegralConfig {
        schwarz_threshold: config.schwarz_threshold.value.into(),
    }
}

fn resolve_random_seeds(config: &mut HfConfig) {
    let guess = &mut config.guess.value;
    match guess {
        crate::config::DensityGuessConfig::Random { config } => {
            resolve_seed(&mut config.random.seed);
        }
        crate::config::DensityGuessConfig::CoreHamiltonian { perturbation }
        | crate::config::DensityGuessConfig::OneElectron { perturbation } => {
            if let Some(perturbation) = perturbation {
                resolve_seed(&mut perturbation.random.seed);
            }
        }
        crate::config::DensityGuessConfig::Zero => {}
    }
}

fn resolve_seed(seed: &mut Option<u64>) {
    if seed.is_none() {
        *seed = Some(rand::random());
    }
}

pub(super) fn normalized_hf_config(config: &HfConfig) -> HfConfig {
    HfConfig {
        method: config.method.value.into(),
        max_iterations: config.max_iterations,
        convergence_threshold: config.convergence_threshold,
        guess: config.guess.value.into(),
        diis: crate::config::DiisConfig {
            enabled: config.diis.enabled,
            max_history: config.diis.max_history.value.into(),
        },
        orthogonalization: crate::config::OrthogonalizationConfig {
            linear_dependency_threshold: config
                .orthogonalization
                .linear_dependency_threshold
                .value
                .into(),
        },
    }
}

impl CalculationExecution for CalculationBuilder<'_> {
    fn execute_with_events(
        &self,
        mut events: impl FnMut(CalculationEvent<'_>),
    ) -> Result<CalculationResult, CalculationExecutionError> {
        self.prepare_with_events(&mut events)?
            .execute_with_events(events)
    }
}
