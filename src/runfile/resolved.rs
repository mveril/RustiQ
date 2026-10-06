//! Fully resolved frontend values, independent of input syntax and source locations.
use serde::{Deserialize, Serialize};
use std::{num::NonZeroU8, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedCalculationConfig {
    pub molecule: ResolvedMoleculeConfig,
    pub basis: ResolvedBasisConfig,
    pub method: ResolvedMethodConfig,
    pub integrals: ResolvedIntegralConfig,
    pub cache: ResolvedCacheConfig,
    pub output: ResolvedOutputConfig,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedMoleculeConfig {
    pub geometry: PathBuf,
    pub charge: i32,
    pub multiplicity: NonZeroU8,
    pub units: Units,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum Units {
    Angstrom,
    Bohr,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedBasisConfig {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedMethodConfig {
    pub hf: ResolvedHfConfig,
    pub mp2: Option<ResolvedMp2Config>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedHfConfig {
    pub method: HfMethod,
    pub max_iterations: std::num::NonZeroUsize,
    pub convergence_threshold: f64,
    pub guess: Guess,
    pub diis: ResolvedDiisConfig,
    pub orthogonalization: ResolvedOrthogonalizationConfig,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum HfMethod {
    Auto,
    Rhf,
    Uhf,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
// Serde does not support deny_unknown_fields alongside flatten. Nickel checks
// the closed guess records before they cross this boundary.
#[serde(tag = "type")]
pub enum Guess {
    CoreHamiltonian {
        perturbation: Option<Distribution>,
    },
    OneElectron {
        perturbation: Option<Distribution>,
    },
    Random {
        #[serde(flatten)]
        distribution: Distribution,
    },
    Zero,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "distribution", deny_unknown_fields)]
pub enum Distribution {
    Normal {
        mean: f64,
        std_dev: f64,
        seed: Option<u64>,
    },
    Uniform {
        min: f64,
        max: f64,
        seed: Option<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedDiisConfig {
    pub enabled: bool,
    pub max_history: usize,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedOrthogonalizationConfig {
    pub linear_dependency_threshold: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedMp2Config {
    pub frozen_orbitals: usize,
    pub memory_limit: super::mp2::MemoryLimit,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedIntegralConfig {
    pub schwarz_threshold: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedCacheConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedOutputConfig {
    pub scf: ScfOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy, Deserialize, Serialize)]
pub enum ScfOutput {
    Normal,
    Quiet,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedInput {
    calculations: Vec<ResolvedCalculationConfig>,
}

#[derive(Debug, thiserror::Error)]
pub enum ResolutionError {
    #[error("at least one calculation is required")]
    EmptyInput,
    #[error("execution of {count} calculations is not yet supported")]
    UnsupportedBatch { count: usize },
    #[error("resolved frontend contract drift at {field}: {message}")]
    ContractDrift {
        field: &'static str,
        message: String,
    },
}

fn drift(field: &'static str, error: impl std::fmt::Display) -> ResolutionError {
    ResolutionError::ContractDrift {
        field,
        message: error.to_string(),
    }
}

impl ResolvedInput {
    pub fn new(calculations: Vec<ResolvedCalculationConfig>) -> Result<Self, ResolutionError> {
        if calculations.is_empty() {
            return Err(ResolutionError::EmptyInput);
        }
        Ok(Self { calculations })
    }

    pub fn calculations(&self) -> &[ResolvedCalculationConfig] {
        &self.calculations
    }

    pub fn single_calculation(&self) -> Result<&ResolvedCalculationConfig, ResolutionError> {
        match self.calculations() {
            [calculation] => Ok(calculation),
            calculations => Err(ResolutionError::UnsupportedBatch {
                count: calculations.len(),
            }),
        }
    }
}

// Keep the invariant even when the embedded evaluator crosses the Serde boundary.
impl<'de> serde::Deserialize<'de> for ResolvedInput {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Transport {
            calculations: Vec<ResolvedCalculationConfig>,
        }
        let transport = Transport::deserialize(deserializer)?;
        Self::new(transport.calculations).map_err(serde::de::Error::custom)
    }
}

impl ResolvedCalculationConfig {
    // Temporary private migration bridge: the production TOML parser still owns defaults.
    pub(super) fn from_runfile(runfile: &super::RunFile) -> miette::Result<Self> {
        use miette::IntoDiagnostic;
        let source = toml_spanner::to_string(runfile).into_diagnostic()?;
        let mut value: toml::Value = toml::from_str(&source).into_diagnostic()?;
        let method = value
            .as_table_mut()
            .ok_or_else(|| miette::miette!("expected resolved record"))?
            .entry("method")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if method.get("hf").is_none() {
            let hf = toml_spanner::to_string(&super::hf::HfConfig::default()).into_diagnostic()?;
            method
                .as_table_mut()
                .ok_or_else(|| miette::miette!("expected method record"))?
                .insert("hf".into(), toml::from_str(&hf).into_diagnostic()?);
        }
        value.try_into().into_diagnostic()
    }

    pub fn resource_path(&self, input: Option<&std::path::Path>) -> PathBuf {
        input
            .and_then(std::path::Path::parent)
            .unwrap_or_else(|| std::path::Path::new("."))
            .join(&self.molecule.geometry)
    }

    pub fn molecule_config(&self) -> rustiq_core::config::MoleculeConfig {
        use rustiq_core::molecules::units::Units as CoreUnits;
        rustiq_core::config::MoleculeConfig {
            units: match self.molecule.units {
                Units::Bohr => CoreUnits::Bohr,
                Units::Angstrom => CoreUnits::Angstrom,
            },
            charge: self.molecule.charge.into(),
            multiplicity: self.molecule.multiplicity.into(),
        }
    }

    pub fn integral_config(&self) -> Result<rustiq_core::config::IntegralConfig, ResolutionError> {
        Ok(rustiq_core::config::IntegralConfig {
            schwarz_threshold: if self.integrals.schwarz_threshold == 0.0 {
                None
            } else {
                Some(
                    rustiq_core::config::validated::PositiveFiniteF64::try_new(
                        self.integrals.schwarz_threshold,
                    )
                    .map_err(|e| drift("integrals.schwarz_threshold", e))?,
                )
            }
            .into(),
        })
    }

    pub fn hf_config(&self) -> Result<rustiq_core::config::HfConfig, ResolutionError> {
        use rustiq_core::config::{
            self as core,
            validated::{DiisSize, NonNegativeFiniteF64, PositiveFiniteF64},
        };
        let hf = &self.method.hf;
        let perturbation = |value: &Option<Distribution>| -> Result<_, ResolutionError> {
            value
                .as_ref()
                .map(|value| {
                    Ok(core::GuessPerturbationConfig {
                        random: value.to_core()?,
                    })
                })
                .transpose()
        };
        let guess = match &hf.guess {
            Guess::CoreHamiltonian {
                perturbation: value,
            } => core::DensityGuessConfig::CoreHamiltonian {
                perturbation: perturbation(value)?,
            },
            Guess::OneElectron {
                perturbation: value,
            } => core::DensityGuessConfig::OneElectron {
                perturbation: perturbation(value)?,
            },
            Guess::Random { distribution } => core::DensityGuessConfig::Random {
                config: core::RandomGuessConfig {
                    random: distribution.to_core()?,
                },
            },
            Guess::Zero => core::DensityGuessConfig::Zero,
        };
        Ok(core::HfConfig {
            method: match hf.method {
                HfMethod::Auto => core::HfMethod::Auto,
                HfMethod::Rhf => core::HfMethod::Rhf,
                HfMethod::Uhf => core::HfMethod::Uhf,
            }
            .into(),
            max_iterations: hf.max_iterations,
            convergence_threshold: PositiveFiniteF64::try_new(hf.convergence_threshold)
                .map_err(|e| drift("hf.convergence_threshold", e))?,
            guess: guess.into(),
            diis: core::DiisConfig {
                enabled: hf.diis.enabled,
                max_history: DiisSize::try_new(hf.diis.max_history)
                    .map_err(|e| drift("diis.max_history", e))?
                    .into(),
            },
            orthogonalization: core::OrthogonalizationConfig {
                linear_dependency_threshold: NonNegativeFiniteF64::try_new(
                    hf.orthogonalization.linear_dependency_threshold,
                )
                .map_err(|e| drift("orthogonalization.linear_dependency_threshold", e))?
                .into(),
            },
        })
    }

    pub fn mp2_config(&self) -> Option<rustiq_core::config::Mp2Config> {
        self.method
            .mp2
            .as_ref()
            .map(|mp2| rustiq_core::config::Mp2Config {
                frozen_orbitals: mp2.frozen_orbitals.into(),
                memory_limit: match mp2.memory_limit {
                    super::mp2::MemoryLimit::Auto => rustiq_core::config::MemoryLimit::Auto,
                    super::mp2::MemoryLimit::Fixed(size) => {
                        rustiq_core::config::MemoryLimit::Fixed(size)
                    }
                }
                .into(),
            })
    }
}

impl Distribution {
    fn to_core(&self) -> Result<rustiq_core::config::random_config::RandomConfig, ResolutionError> {
        use rustiq_core::config::random_config::{
            self as core,
            distribution_config::{
                DistributionConfig, NormalDistributionConfig, UniformDistributionConfig,
            },
        };
        let (distribution, seed) = match *self {
            Self::Normal {
                mean,
                std_dev,
                seed,
            } => {
                if !mean.is_finite() || !std_dev.is_finite() || std_dev <= 0.0 {
                    return Err(drift("guess.distribution", "invalid normal parameters"));
                }
                (
                    DistributionConfig::Normal {
                        config: NormalDistributionConfig {
                            mean,
                            std_dev: rustiq_core::config::validated::PositiveFiniteF64::try_new(
                                std_dev,
                            )
                            .map_err(|e| drift("guess.std_dev", e))?,
                        },
                    },
                    seed,
                )
            }
            Self::Uniform { min, max, seed } => {
                if !min.is_finite() || !max.is_finite() || max <= min || !(max - min).is_finite() {
                    return Err(drift("guess.distribution", "invalid uniform bounds"));
                }
                (
                    DistributionConfig::Uniform {
                        config: UniformDistributionConfig { min, max },
                    },
                    seed,
                )
            }
        };
        Ok(core::RandomConfig { distribution, seed })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved() -> ResolvedCalculationConfig {
        super::super::parser::parse_runfile("input.toml", "[basis]\nname = \"sto-3g\"\n")
            .unwrap()
            .resolved
            .calculations()[0]
            .clone()
    }

    #[test]
    fn empty_inputs_cannot_cross_boundary() {
        assert!(ResolvedInput::new(vec![]).is_err());
        assert!(serde_json::from_str::<ResolvedInput>(r#"{"calculations":[]}"#).is_err());
    }

    #[test]
    fn core_conversion_reports_contract_drift() {
        let mut calculation = resolved();
        calculation.method.hf.convergence_threshold = f64::NAN;
        assert!(calculation.hf_config().is_err());
        calculation = resolved();
        calculation.method.hf.diis.max_history = 1;
        assert!(calculation.hf_config().is_err());
        calculation.integrals.schwarz_threshold = -1.0;
        assert!(calculation.integral_config().is_err());
        calculation = resolved();
        calculation.method.hf.guess = Guess::Random {
            distribution: Distribution::Uniform {
                min: 1.0,
                max: 0.0,
                seed: None,
            },
        };
        assert!(calculation.hf_config().is_err());
    }

    #[test]
    fn resource_paths_follow_input_without_changing_working_directory() {
        let cwd = std::env::current_dir().unwrap();
        let mut calculation = resolved();
        calculation.molecule.geometry = "../molecule.xyz".into();
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("inputs");
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(temporary.path().join("molecule.xyz"), "geometry").unwrap();
        let path = calculation.resource_path(Some(&directory.join("calculation.toml")));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "geometry");
        assert_eq!(
            calculation.resource_path(None),
            std::path::Path::new(".").join("../molecule.xyz")
        );
        assert_eq!(std::env::current_dir().unwrap(), cwd);
        calculation.molecule.geometry = temporary.path().join("molecule.xyz");
        assert_eq!(
            calculation.resource_path(Some(&directory.join("calculation.toml"))),
            calculation.molecule.geometry
        );
    }

    #[test]
    fn unsupported_batches_are_rejected_without_selecting_the_first_entry() {
        let parsed =
            super::super::parser::parse_runfile("input.toml", "[basis]\nname = 'sto-3g'\n")
                .unwrap();
        let calculation = parsed.resolved.single_calculation().unwrap().clone();
        let batch = ResolvedInput::new(vec![calculation.clone(), calculation]).unwrap();
        assert!(matches!(
            batch.single_calculation(),
            Err(ResolutionError::UnsupportedBatch { count: 2 })
        ));
        assert_eq!(batch.calculations().len(), 2);
    }

    #[test]
    fn required_resolved_fields_do_not_receive_serde_defaults() {
        let parsed =
            super::super::parser::parse_runfile("input.toml", "[basis]\nname = 'sto-3g'\n")
                .unwrap();
        let value = serde_json::to_value(parsed.resolved.single_calculation().unwrap()).unwrap();
        for (section, field) in [
            ("molecule", "units"),
            ("method", "hf"),
            ("integrals", "schwarz_threshold"),
            ("cache", "enabled"),
            ("output", "scf"),
        ] {
            let mut incomplete = value.clone();
            incomplete[section].as_object_mut().unwrap().remove(field);
            assert!(
                serde_json::from_value::<ResolvedCalculationConfig>(incomplete).is_err(),
                "{section}.{field}"
            );
        }
    }
}
