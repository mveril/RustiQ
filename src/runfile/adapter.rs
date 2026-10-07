//! Explicit conversions between CLI TOML representations and scientific options.
use super::{hf, integrals, molecule, mp2, random_config};
use rustiq_core::config as core;

impl From<&molecule::MoleculeConfig> for core::MoleculeConfig {
    fn from(value: &molecule::MoleculeConfig) -> Self {
        Self {
            units: value.units,
            charge: value.charge.into(),
            multiplicity: value.multiplicity.into(),
        }
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
        Self {
            enabled: value.enabled,
            max_history: value.max_history.into(),
        }
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
        Self {
            linear_dependency_threshold: value.linear_dependency_threshold.into(),
        }
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
        Self {
            method: core::HfMethod::from(&value.method).into(),
            max_iterations: value.max_iterations,
            convergence_threshold: value.convergence_threshold,
            guess: core::DensityGuessConfig::from(value.guess).into(),
            diis: (&value.diis).into(),
            orthogonalization: (&value.orthogonalization).into(),
        }
    }
}

impl From<&mp2::Mp2Config> for core::Mp2Config {
    fn from(value: &mp2::Mp2Config) -> Self {
        Self {
            frozen_orbitals: value.frozen_orbitals.into(),
            memory_limit: match value.memory_limit {
                crate::config::MemoryLimit::Auto => core::MemoryLimit::Auto,
                crate::config::MemoryLimit::Fixed(size) => core::MemoryLimit::Fixed(size),
            }
            .into(),
        }
    }
}

impl From<&integrals::IntegralConfig> for core::IntegralConfig {
    fn from(value: &integrals::IntegralConfig) -> Self {
        Self {
            schwarz_threshold: value.schwarz_threshold.into(),
        }
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
        Self {
            random: value.random.into(),
        }
    }
}

impl From<hf::RandomGuessConfig> for core::RandomGuessConfig {
    fn from(value: hf::RandomGuessConfig) -> Self {
        Self {
            random: value.random.into(),
        }
    }
}

impl From<random_config::RandomConfig> for core::random_config::RandomConfig {
    fn from(value: random_config::RandomConfig) -> Self {
        use core::random_config::distribution_config::{
            NormalDistributionConfig, UniformDistributionConfig,
        };
        use core::random_config::DistributionConfig;
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
        Self {
            distribution,
            seed: value.seed,
        }
    }
}

impl From<&core::HfConfig> for hf::HfConfig {
    fn from(value: &core::HfConfig) -> Self {
        Self {
            method: value.method.value.into(),
            max_iterations: value.max_iterations,
            convergence_threshold: value.convergence_threshold,
            guess: value.guess.value.into(),
            diis: (&value.diis).into(),
            orthogonalization: (&value.orthogonalization).into(),
        }
    }
}

impl From<&core::IntegralConfig> for integrals::IntegralConfig {
    fn from(value: &core::IntegralConfig) -> Self {
        Self {
            schwarz_threshold: value.schwarz_threshold.value,
        }
    }
}

impl From<core::HfMethod> for hf::HfMethod {
    fn from(value: core::HfMethod) -> Self {
        match value {
            core::HfMethod::Auto => Self::Auto,
            core::HfMethod::Rhf => Self::Rhf,
            core::HfMethod::Uhf => Self::Uhf,
        }
    }
}

impl From<core::DensityGuessConfig> for hf::DensityGuessConfig {
    fn from(value: core::DensityGuessConfig) -> Self {
        match value {
            core::DensityGuessConfig::CoreHamiltonian { perturbation } => Self::CoreHamiltonian {
                perturbation: perturbation.map(|value| hf::GuessPerturbationConfig {
                    random: value.random.into(),
                }),
            },
            core::DensityGuessConfig::OneElectron { perturbation } => Self::OneElectron {
                perturbation: perturbation.map(|value| hf::GuessPerturbationConfig {
                    random: value.random.into(),
                }),
            },
            core::DensityGuessConfig::Random { config } => Self::Random {
                config: hf::RandomGuessConfig {
                    random: config.random.into(),
                },
            },
            core::DensityGuessConfig::Zero => Self::Zero,
        }
    }
}

impl From<core::random_config::RandomConfig> for random_config::RandomConfig {
    fn from(value: core::random_config::RandomConfig) -> Self {
        use random_config::distribution_config::{
            NormalDistributionConfig, UniformDistributionConfig,
        };
        let distribution = match value.distribution {
            core::random_config::DistributionConfig::Normal { config } => {
                random_config::DistributionConfig::Normal {
                    config: NormalDistributionConfig {
                        mean: config.mean,
                        std_dev: config.std_dev,
                    },
                }
            }
            core::random_config::DistributionConfig::Uniform { config } => {
                random_config::DistributionConfig::Uniform {
                    config: UniformDistributionConfig {
                        min: config.min,
                        max: config.max,
                    },
                }
            }
        };
        Self {
            distribution,
            seed: value.seed,
        }
    }
}

impl From<&core::Mp2Config> for mp2::Mp2Config {
    fn from(value: &core::Mp2Config) -> Self {
        Self {
            frozen_orbitals: value.frozen_orbitals.value,
            memory_limit: match value.memory_limit.value {
                core::MemoryLimit::Auto => crate::config::MemoryLimit::Auto,
                core::MemoryLimit::Fixed(size) => crate::config::MemoryLimit::Fixed(size),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use rustiq_core::config::{DensityGuessConfig, RandomGuessConfig};
    use std::mem::discriminant;

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
        let core_diis = rustiq_core::config::DiisConfig {
            max_history: rustiq_core::config::Located {
                value: core_diis.max_history.value,
                span: Some((12usize, 2usize).into()),
            },
            ..core_diis
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
        let core_orthogonalization = rustiq_core::config::OrthogonalizationConfig {
            linear_dependency_threshold: rustiq_core::config::Located {
                value: core_orthogonalization.linear_dependency_threshold.value,
                span: Some((20usize, 3usize).into()),
            },
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
