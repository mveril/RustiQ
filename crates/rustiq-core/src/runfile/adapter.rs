//! Explicit conversion from the TOML schema to scientific options.
use super::{hf, mp2, random_config};
use crate::config as core;

// Serialization belongs to the optional frontend, including for domain enums.
#[derive(toml_spanner::Toml)]
#[toml(Toml)]
enum UnitsRepr {
    Bohr,
    Angstrom,
}

impl<'de> toml_spanner::FromToml<'de> for crate::molecules::units::Units {
    fn from_toml(
        ctx: &mut toml_spanner::Context<'de>,
        item: &toml_spanner::Item<'de>,
    ) -> Result<Self, toml_spanner::Failed> {
        Ok(match UnitsRepr::from_toml(ctx, item)? {
            UnitsRepr::Bohr => Self::Bohr,
            UnitsRepr::Angstrom => Self::Angstrom,
        })
    }
}

impl toml_spanner::ToToml for crate::molecules::units::Units {
    fn to_toml<'a>(
        &'a self,
        arena: &'a toml_spanner::Arena,
    ) -> Result<toml_spanner::Item<'a>, toml_spanner::ToTomlError> {
        match self {
            Self::Bohr => UnitsRepr::Bohr.to_toml(arena),
            Self::Angstrom => UnitsRepr::Angstrom.to_toml(arena),
        }
    }
}

impl From<&super::global::molecule_config::MoleculeConfig> for core::MoleculeConfig {
    fn from(value: &super::global::molecule_config::MoleculeConfig) -> Self {
        Self {
            units: value.molecule_unit,
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

impl From<&hf::HfConfig> for core::HfConfig {
    fn from(value: &hf::HfConfig) -> Self {
        Self {
            method: core::HfMethod::from(&value.method).into(),
            max_iterations: value.max_iterations,
            convergence_threshold: value.convergence_threshold,
            linear_dependency_threshold: value.linear_dependency_threshold.into(),
            guess: core::DensityGuessConfig::from(value.guess).into(),
            diis: value.diis,
            diis_size: value.diis_size,
        }
    }
}

impl From<&mp2::Mp2Config> for core::Mp2Config {
    fn from(value: &mp2::Mp2Config) -> Self {
        Self {
            frozen_orbitals: value.frozen_orbitals.into(),
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

#[cfg(test)]
mod tests {
    use crate::config::{DensityGuessConfig, RandomGuessConfig};
    use std::mem::discriminant;
    use toml_spanner::Toml;
    #[test]
    fn test_density_guess_type_deserialization() {
        #[derive(Toml)]
        #[toml(FromToml)]
        struct GuessConfig {
            guess: crate::runfile::hf::DensityGuessConfig,
        }

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
            let config: GuessConfig = toml_spanner::from_str(toml).unwrap();
            assert_eq!(
                discriminant(&crate::config::DensityGuessConfig::from(config.guess)),
                discriminant(&expected)
            );
            let _density_guess = config.guess;
        }
    }
}
