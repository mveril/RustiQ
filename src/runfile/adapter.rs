//! Explicit conversions between CLI TOML representations and scientific options.
use super::{hf, integrals, molecule, mp2, random_config};
use rustiq_core::config as core;

#[derive(Debug, thiserror::Error)]
#[error("the CLI cannot render an unknown {0} variant from rustiq-core")]
pub struct UnsupportedCoreVariant(&'static str);

impl From<&molecule::MoleculeConfig> for core::MoleculeConfig {
    fn from(value: &molecule::MoleculeConfig) -> Self {
        let mut config = Self::default();
        config.units = value.units;
        config.charge = value.charge.into();
        config.multiplicity = value.multiplicity.into();
        config
    }
}

impl From<&hf::HfMethod> for core::HfMethod {
    fn from(value: &hf::HfMethod) -> Self {
        match value {
            hf::HfMethod::Auto => Self::Auto,
            hf::HfMethod::Rhf => Self::Rhf,
            hf::HfMethod::Uhf => Self::Uhf,
        }
    }
}

impl From<&hf::DiisConfig> for core::DiisConfig {
    fn from(value: &hf::DiisConfig) -> Self {
        let mut config = Self::default();
        config.enabled = value.enabled;
        config.max_history = value.max_history.into();
        config
    }
}

impl From<&core::DiisConfig> for hf::DiisConfig {
    fn from(value: &core::DiisConfig) -> Self {
        Self {
            enabled: value.enabled,
            max_history: value.max_history.value,
        }
    }
}

impl From<&hf::OrthogonalizationConfig> for core::OrthogonalizationConfig {
    fn from(value: &hf::OrthogonalizationConfig) -> Self {
        let mut config = Self::default();
        config.linear_dependency_threshold = value.linear_dependency_threshold.into();
        config
    }
}

impl From<&core::OrthogonalizationConfig> for hf::OrthogonalizationConfig {
    fn from(value: &core::OrthogonalizationConfig) -> Self {
        Self {
            linear_dependency_threshold: value.linear_dependency_threshold.value,
        }
    }
}

impl From<&hf::HfConfig> for core::HfConfig {
    fn from(value: &hf::HfConfig) -> Self {
        let mut config = Self::default();
        config.method = core::HfMethod::from(&value.method).into();
        config.max_iterations = value.max_iterations;
        config.convergence_threshold = value.convergence_threshold;
        config.guess = core::DensityGuessConfig::from(value.guess).into();
        config.diis = (&value.diis).into();
        config.orthogonalization = (&value.orthogonalization).into();
        config
    }
}

impl From<&mp2::Mp2Config> for core::Mp2Config {
    fn from(value: &mp2::Mp2Config) -> Self {
        let mut config = Self::default();
        config.frozen_orbitals = value.frozen_orbitals.into();
        config.memory_limit = match value.memory_limit {
            crate::config::MemoryLimit::Auto => core::MemoryLimit::Auto,
            crate::config::MemoryLimit::Fixed(size) => core::MemoryLimit::Fixed(size),
        }
        .into();
        config
    }
}

impl From<&integrals::IntegralConfig> for core::IntegralConfig {
    fn from(value: &integrals::IntegralConfig) -> Self {
        let mut config = Self::default();
        config.schwarz_threshold = value.schwarz_threshold.into();
        config
    }
}

impl From<hf::DensityGuessConfig> for core::DensityGuessConfig {
    fn from(value: hf::DensityGuessConfig) -> Self {
        match value {
            hf::DensityGuessConfig::CoreHamiltonian { perturbation } => Self::CoreHamiltonian {
                perturbation: perturbation.map(Into::into),
            },
            hf::DensityGuessConfig::OneElectron { perturbation } => Self::OneElectron {
                perturbation: perturbation.map(Into::into),
            },
            hf::DensityGuessConfig::Random { config } => Self::Random {
                config: config.into(),
            },
            hf::DensityGuessConfig::Zero => Self::Zero,
        }
    }
}

impl From<hf::GuessPerturbationConfig> for core::GuessPerturbationConfig {
    fn from(value: hf::GuessPerturbationConfig) -> Self {
        let mut config = Self::default();
        config.random = value.random.into();
        config
    }
}

impl From<hf::RandomGuessConfig> for core::RandomGuessConfig {
    fn from(value: hf::RandomGuessConfig) -> Self {
        let mut config = Self::default();
        config.random = value.random.into();
        config
    }
}

impl From<random_config::RandomConfig> for core::RandomConfig {
    fn from(value: random_config::RandomConfig) -> Self {
        use core::DistributionConfig;
        use core::{NormalDistributionConfig, UniformDistributionConfig};
        let distribution = match value.distribution {
            random_config::DistributionConfig::Normal { config } => DistributionConfig::Normal {
                config: NormalDistributionConfig {
                    mean: config.mean,
                    std_dev: config.std_dev,
                },
            },
            random_config::DistributionConfig::Uniform { config } => DistributionConfig::Uniform {
                config: UniformDistributionConfig {
                    min: config.min,
                    max: config.max,
                },
            },
        };
        let mut config = Self::new(distribution);
        config.seed = value.seed;
        config
    }
}

impl TryFrom<&core::HfConfig> for hf::HfConfig {
    type Error = UnsupportedCoreVariant;

    fn try_from(value: &core::HfConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            method: value.method.value.try_into()?,
            max_iterations: value.max_iterations,
            convergence_threshold: value.convergence_threshold,
            guess: value.guess.value.try_into()?,
            diis: (&value.diis).into(),
            orthogonalization: (&value.orthogonalization).into(),
        })
    }
}

impl From<&core::IntegralConfig> for integrals::IntegralConfig {
    fn from(value: &core::IntegralConfig) -> Self {
        Self {
            schwarz_threshold: value.schwarz_threshold.value,
        }
    }
}

impl TryFrom<core::HfMethod> for hf::HfMethod {
    type Error = UnsupportedCoreVariant;

    fn try_from(value: core::HfMethod) -> Result<Self, Self::Error> {
        match value {
            core::HfMethod::Auto => Ok(Self::Auto),
            core::HfMethod::Rhf => Ok(Self::Rhf),
            core::HfMethod::Uhf => Ok(Self::Uhf),
            _ => Err(UnsupportedCoreVariant("HF method")),
        }
    }
}

impl TryFrom<core::DensityGuessConfig> for hf::DensityGuessConfig {
    type Error = UnsupportedCoreVariant;

    fn try_from(value: core::DensityGuessConfig) -> Result<Self, Self::Error> {
        match value {
            core::DensityGuessConfig::CoreHamiltonian { perturbation } => {
                Ok(Self::CoreHamiltonian {
                    perturbation: convert_perturbation(perturbation)?,
                })
            }
            core::DensityGuessConfig::OneElectron { perturbation } => Ok(Self::OneElectron {
                perturbation: convert_perturbation(perturbation)?,
            }),
            core::DensityGuessConfig::Random { config } => Ok(Self::Random {
                config: hf::RandomGuessConfig {
                    random: config.random.try_into()?,
                },
            }),
            core::DensityGuessConfig::Zero => Ok(Self::Zero),
            _ => Err(UnsupportedCoreVariant("density guess")),
        }
    }
}

fn convert_perturbation(
    value: Option<core::GuessPerturbationConfig>,
) -> Result<Option<hf::GuessPerturbationConfig>, UnsupportedCoreVariant> {
    value
        .map(|value| {
            Ok(hf::GuessPerturbationConfig {
                random: value.random.try_into()?,
            })
        })
        .transpose()
}

impl TryFrom<core::RandomConfig> for random_config::RandomConfig {
    type Error = UnsupportedCoreVariant;

    fn try_from(value: core::RandomConfig) -> Result<Self, Self::Error> {
        use random_config::distribution_config::{
            NormalDistributionConfig, UniformDistributionConfig,
        };
        let distribution = match value.distribution {
            core::DistributionConfig::Normal { config } => {
                random_config::DistributionConfig::Normal {
                    config: NormalDistributionConfig {
                        mean: config.mean,
                        std_dev: config.std_dev,
                    },
                }
            }
            core::DistributionConfig::Uniform { config } => {
                random_config::DistributionConfig::Uniform {
                    config: UniformDistributionConfig {
                        min: config.min,
                        max: config.max,
                    },
                }
            }
            _ => return Err(UnsupportedCoreVariant("random distribution")),
        };
        Ok(Self {
            distribution,
            seed: value.seed,
        })
    }
}

impl TryFrom<&core::Mp2Config> for mp2::Mp2Config {
    type Error = UnsupportedCoreVariant;

    fn try_from(value: &core::Mp2Config) -> Result<Self, Self::Error> {
        Ok(Self {
            frozen_orbitals: value.frozen_orbitals.value,
            memory_limit: match value.memory_limit.value {
                core::MemoryLimit::Auto => crate::config::MemoryLimit::Auto,
                core::MemoryLimit::Fixed(size) => crate::config::MemoryLimit::Fixed(size),
                _ => return Err(UnsupportedCoreVariant("MP2 memory limit")),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use rustiq_core::config::{DensityGuessConfig, RandomGuessConfig};
    use std::mem::discriminant;

    #[test]
    fn omitted_options_preserve_core_and_frontend_defaults() {
        fn json(value: impl serde::Serialize) -> serde_json::Value {
            serde_json::to_value(value).unwrap()
        }

        let parsed = crate::runfile::parser::parse_runfile(
            "defaults.toml",
            "[basis]\nname = 'sto-3g'\n[method.mp2]\n",
        )
        .unwrap();
        assert_eq!(
            json(super::hf::HfConfig::try_from(parsed.hf_config.as_ref().unwrap()).unwrap()),
            json(super::hf::HfConfig::try_from(&rustiq_core::config::HfConfig::default()).unwrap()),
        );
        assert_eq!(
            json(super::mp2::Mp2Config::try_from(parsed.mp2_config.as_ref().unwrap()).unwrap(),),
            json(
                super::mp2::Mp2Config::try_from(&rustiq_core::config::Mp2Config::default())
                    .unwrap(),
            ),
        );
        assert_eq!(
            json(super::integrals::IntegralConfig::from(
                &parsed.integral_config
            )),
            json(super::integrals::IntegralConfig::from(
                &rustiq_core::config::IntegralConfig::default(),
            )),
        );
        let molecule =
            rustiq_core::config::MoleculeConfig::from(&super::molecule::MoleculeConfig::default());
        assert_eq!(parsed.molecule_config.units, molecule.units);
        // The frontend accepts ordinary XYZ coordinates in Angstrom; the core
        // default is atomic units. Preserve both existing conventions.
        assert_eq!(
            parsed.molecule_config.units,
            rustiq_core::molecules::Units::Angstrom
        );
        assert_eq!(
            rustiq_core::config::MoleculeConfig::default().units,
            rustiq_core::molecules::Units::Bohr,
        );
        assert_eq!(parsed.molecule_config.charge.value, molecule.charge.value);
        assert_eq!(
            parsed.molecule_config.multiplicity.value,
            molecule.multiplicity.value,
        );
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "Configuration and portable round trips must preserve literal values and identical execution results exactly"
    )]
    fn component_configs_convert_in_both_directions() {
        let frontend_diis = crate::runfile::hf::DiisConfig {
            enabled: true,
            max_history: crate::runfile::validated::DiisSize::try_new(9).unwrap(),
        };
        let core_diis = rustiq_core::config::DiisConfig::from(&frontend_diis);
        assert!(core_diis.enabled);
        assert_eq!(core_diis.max_history.value.into_inner(), 9);
        let core_diis = {
            let mut diis = core_diis;
            diis.max_history = rustiq_core::config::Located {
                value: core_diis.max_history.value,
                span: Some((12usize, 2usize).into()),
            };
            diis
        };
        let frontend_diis = crate::runfile::hf::DiisConfig::from(&core_diis);
        assert!(frontend_diis.enabled);
        assert_eq!(frontend_diis.max_history.into_inner(), 9);
        assert_eq!(core_diis.max_history.span.unwrap().offset(), 12);

        let frontend_orthogonalization =
            crate::runfile::hf::OrthogonalizationConfig {
                linear_dependency_threshold:
                    crate::runfile::validated::NonNegativeFiniteF64::try_new(1e-7).unwrap(),
            };
        let core_orthogonalization =
            rustiq_core::config::OrthogonalizationConfig::from(&frontend_orthogonalization);
        assert_eq!(
            core_orthogonalization
                .linear_dependency_threshold
                .value
                .into_inner(),
            1e-7
        );
        let core_orthogonalization = {
            let mut orthogonalization = rustiq_core::config::OrthogonalizationConfig::default();
            orthogonalization.linear_dependency_threshold = rustiq_core::config::Located {
                value: core_orthogonalization.linear_dependency_threshold.value,
                span: Some((20usize, 3usize).into()),
            };
            orthogonalization
        };
        let frontend_orthogonalization =
            crate::runfile::hf::OrthogonalizationConfig::from(&core_orthogonalization);
        assert_eq!(
            frontend_orthogonalization
                .linear_dependency_threshold
                .into_inner(),
            1e-7
        );
        assert_eq!(
            core_orthogonalization
                .linear_dependency_threshold
                .span
                .unwrap()
                .offset(),
            20
        );
    }
    #[test]
    fn test_density_guess_type_deserialization() {
        for (toml, expected) in [
            (
                r#"
                [guess]
                type = "OneElectron"
                "#,
                DensityGuessConfig::OneElectron { perturbation: None },
            ),
            (
                r#"
                [guess]
                type = "Random"
                distribution = "Uniform"
                min = -1.0
                max = 1.0
                "#,
                DensityGuessConfig::Random {
                    config: RandomGuessConfig::default(),
                },
            ),
            (
                r#"
                [guess]
                type = "Zero"
                "#,
                DensityGuessConfig::Zero,
            ),
            (
                r#"
                [guess]
                type = "CoreHamiltonian"
                "#,
                DensityGuessConfig::CoreHamiltonian { perturbation: None },
            ),
        ] {
            let config = crate::runfile::parse_section(
                "method.hf",
                &toml.replace("[guess]", "[method.hf.guess]"),
            )
            .unwrap()
            .method
            .hf
            .unwrap();
            assert_eq!(
                discriminant(&rustiq_core::config::DensityGuessConfig::from(config.guess)),
                discriminant(&expected)
            );
        }
    }
}
