use std::num::NonZeroUsize;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use toml_spanner::Toml;

use crate::{
    molecules::molecule::Molecule,
    runfile::validated::{DiisSize, PositiveFiniteF64},
};

mod density_guess_config;
mod guess_perturbation_config;
mod random_guess_config;

pub(crate) use density_guess_config::DensityGuessConfig;
pub(crate) use guess_perturbation_config::GuessPerturbationConfig;
pub(crate) use random_guess_config::RandomGuessConfig;

#[derive(Debug, Toml)]
#[toml(Toml, recoverable)]
pub(crate) struct HfConfig {
    #[toml(default)]
    pub method: HfMethod,
    #[toml(default = default_max_iter())]
    #[toml(with = crate::runfile::validated::non_zero_usize)]
    pub max_iterations: NonZeroUsize,
    #[toml(default = default_conv_threshold())]
    pub convergence_threshold: PositiveFiniteF64,
    #[toml(default)]
    pub guess: DensityGuessConfig,
    #[toml(default)]
    pub diis: bool,
    #[toml(default = default_diis_size())]
    pub diis_size: DiisSize,
    #[toml(default)]
    pub format: HfOutputFormat,
}

#[derive(Debug, Default, Serialize, Deserialize, Toml, PartialEq, Eq)]
#[toml(Toml)]
pub(crate) enum HfMethod {
    #[default]
    Auto,
    Rhf,
    Uhf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolvedHfMethod {
    Rhf,
    Uhf,
}

impl std::fmt::Display for ResolvedHfMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rhf => write!(f, "RHF"),
            Self::Uhf => write!(f, "UHF"),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum HfMethodResolutionError {
    #[error("invalid electron configuration: total electrons = {electrons}, multiplicity = {multiplicity}")]
    InvalidElectronConfiguration { electrons: usize, multiplicity: u8 },
    #[error("RHF requires a closed-shell singlet: total electrons = {electrons}, multiplicity = {multiplicity}")]
    RhfRequiresClosedShellSinglet { electrons: usize, multiplicity: u8 },
}

impl HfMethod {
    pub(crate) fn resolve(
        &self,
        molecule: &Molecule,
    ) -> Result<ResolvedHfMethod, HfMethodResolutionError> {
        validate_electron_configuration(molecule)?;
        Ok(match self {
            Self::Rhf => {
                if !is_closed_shell_singlet(molecule) {
                    return Err(HfMethodResolutionError::RhfRequiresClosedShellSinglet {
                        electrons: molecule.total_electrons(),
                        multiplicity: molecule.multiplicity.get(),
                    });
                }
                ResolvedHfMethod::Rhf
            }
            Self::Uhf => ResolvedHfMethod::Uhf,
            Self::Auto => {
                if is_closed_shell_singlet(molecule) {
                    ResolvedHfMethod::Rhf
                } else {
                    ResolvedHfMethod::Uhf
                }
            }
        })
    }
}

#[derive(Debug, Default, Toml, PartialEq, Eq)]
#[toml(Toml)]
pub(crate) enum HfOutputFormat {
    #[default]
    Normal,
    Nope,
}

fn default_conv_threshold() -> PositiveFiniteF64 {
    PositiveFiniteF64::try_new(1e-8).expect("default convergence threshold is positive and finite")
}

fn default_max_iter() -> NonZeroUsize {
    NonZeroUsize::new(100).expect("default max iterations is non-zero")
}

fn default_diis_size() -> DiisSize {
    DiisSize::try_new(6).expect("default DIIS history size is at least 2")
}

fn validate_electron_configuration(molecule: &Molecule) -> Result<(), HfMethodResolutionError> {
    let electrons = molecule.total_electrons();
    let spin = molecule.unpaired_electrons() as usize;
    if spin > electrons || (electrons + spin) % 2 != 0 {
        return Err(HfMethodResolutionError::InvalidElectronConfiguration {
            electrons,
            multiplicity: molecule.multiplicity.get(),
        });
    }
    Ok(())
}

fn is_closed_shell_singlet(molecule: &Molecule) -> bool {
    molecule.multiplicity.get() == 1 && molecule.total_electrons() % 2 == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::molecules::{atom::Atom, geometry::Geometry, molecule::Molecule, units::Units};
    use crate::runfile::random_config::DistributionConfig;
    use nalgebra::point;
    use std::mem::discriminant;
    use std::num::NonZeroU8;

    fn molecule(atom_symbols: &[&str], charge: i32, multiplicity: u8) -> Molecule {
        let elements = periodic_table::periodic_table();
        let atoms = atom_symbols
            .iter()
            .enumerate()
            .map(|(index, symbol)| {
                let element = elements
                    .iter()
                    .find(|element| element.symbol == *symbol)
                    .unwrap();
                Atom::new(element, point![0.0, 0.0, index as f64])
            })
            .collect();
        // SAFETY: These tests only build molecules whose charge does not exceed
        // their nuclear charge.
        unsafe {
            Molecule::new_unchecked(
                Geometry::new("test molecule".to_string(), atoms),
                Units::Bohr,
                charge,
                NonZeroU8::new(multiplicity).unwrap(),
            )
        }
    }

    #[test]
    fn test_hf_config_diis_defaults() {
        let config: HfConfig = toml_spanner::from_str("").unwrap();

        assert!(!config.diis);
        assert_eq!(config.method, HfMethod::Auto);
        assert_eq!(config.max_iterations.get(), 100);
        assert_eq!(config.convergence_threshold.into_inner(), 1e-8);
        assert_eq!(config.diis_size.into_inner(), 6);
        assert_eq!(config.format, HfOutputFormat::Normal);
    }

    #[test]
    fn test_hf_config_method_deserialization() {
        let auto: HfConfig = toml_spanner::from_str(r#"method = "Auto""#).unwrap();
        let rhf: HfConfig = toml_spanner::from_str(r#"method = "Rhf""#).unwrap();
        let uhf: HfConfig = toml_spanner::from_str(r#"method = "Uhf""#).unwrap();

        assert_eq!(auto.method, HfMethod::Auto);
        assert_eq!(rhf.method, HfMethod::Rhf);
        assert_eq!(uhf.method, HfMethod::Uhf);
    }

    #[test]
    fn test_hf_auto_resolves_closed_shell_singlet_to_rhf() {
        let molecule = molecule(&["H", "H"], 0, 1);

        assert_eq!(
            HfMethod::Auto.resolve(&molecule).unwrap(),
            ResolvedHfMethod::Rhf
        );
    }

    #[test]
    fn test_hf_auto_resolves_open_shell_to_uhf() {
        let hydroxyl = molecule(&["O", "H"], 0, 2);
        let cation = molecule(&["H", "H"], 1, 2);

        assert_eq!(
            HfMethod::Auto.resolve(&hydroxyl).unwrap(),
            ResolvedHfMethod::Uhf
        );
        assert_eq!(
            HfMethod::Auto.resolve(&cation).unwrap(),
            ResolvedHfMethod::Uhf
        );
    }

    #[test]
    fn test_hf_explicit_methods_resolve_without_auto_selection() {
        let molecule = molecule(&["H", "H"], 0, 1);

        assert_eq!(
            HfMethod::Rhf.resolve(&molecule).unwrap(),
            ResolvedHfMethod::Rhf
        );
        assert_eq!(
            HfMethod::Uhf.resolve(&molecule).unwrap(),
            ResolvedHfMethod::Uhf
        );
    }

    #[test]
    fn test_hf_explicit_rhf_rejects_open_shell_molecule() {
        let hydroxyl = molecule(&["O", "H"], 0, 2);

        assert_eq!(
            HfMethod::Rhf.resolve(&hydroxyl).unwrap_err(),
            HfMethodResolutionError::RhfRequiresClosedShellSinglet {
                electrons: 9,
                multiplicity: 2
            }
        );
    }

    #[test]
    fn test_hf_method_resolution_rejects_incompatible_multiplicity() {
        let molecule = molecule(&["H", "H"], 0, 2);

        assert_eq!(
            HfMethod::Auto.resolve(&molecule).unwrap_err(),
            HfMethodResolutionError::InvalidElectronConfiguration {
                electrons: 2,
                multiplicity: 2
            }
        );
    }

    #[test]
    fn test_hf_config_diis_deserialization() {
        let config: HfConfig = toml_spanner::from_str(
            r#"
            diis = true
            diis_size = 8
            "#,
        )
        .unwrap();

        assert!(config.diis);
        assert_eq!(config.diis_size.into_inner(), 8);
    }

    #[test]
    fn test_hf_config_format_deserialization() {
        let normal: HfConfig = toml_spanner::from_str(r#"format = "Normal""#).unwrap();
        let nope: HfConfig = toml_spanner::from_str(r#"format = "Nope""#).unwrap();

        assert_eq!(normal.format, HfOutputFormat::Normal);
        assert_eq!(nope.format, HfOutputFormat::Nope);
    }

    #[test]
    fn test_hf_config_random_guess_deserialization() {
        let config: HfConfig = toml_spanner::from_str(
            r#"
            [guess]
            type = "Random"
            distribution = "Normal"
            mean = 0.0
            std_dev = 0.5
            seed = 42
            "#,
        )
        .unwrap();

        assert_eq!(
            discriminant(&config.guess),
            discriminant(&DensityGuessConfig::Random {
                config: RandomGuessConfig::default()
            })
        );
        let DensityGuessConfig::Random {
            config: guess_config,
        } = config.guess
        else {
            panic!("expected random guess config");
        };
        assert_eq!(guess_config.random.seed, Some(42));
    }

    #[test]
    fn test_hf_config_core_guess_perturbation_deserialization() {
        let config: HfConfig = toml_spanner::from_str(
            r#"
            [guess]
            type = "CoreHamiltonian"

            [guess.perturbation]
            distribution = "Normal"
            mean = 0.0
            std_dev = 1e-4
            seed = 42
            "#,
        )
        .unwrap();

        let DensityGuessConfig::CoreHamiltonian {
            perturbation: Some(perturbation),
        } = config.guess
        else {
            panic!("expected perturbed core hamiltonian guess config");
        };
        assert_eq!(perturbation.random.seed, Some(42));
    }

    #[test]
    fn test_hf_config_guess_perturbation_defaults() {
        let config: HfConfig = toml_spanner::from_str(
            r#"
            [guess]
            type = "CoreHamiltonian"

            [guess.perturbation]
            "#,
        )
        .unwrap();

        let DensityGuessConfig::CoreHamiltonian {
            perturbation: Some(perturbation),
        } = config.guess
        else {
            panic!("expected default perturbation config");
        };
        assert_eq!(perturbation.random.seed, None);
        match perturbation.random.distribution {
            DistributionConfig::Normal { config } => {
                assert_eq!(config.mean, 0.0);
                assert_eq!(config.std_dev.into_inner(), 1e-4);
            }
            DistributionConfig::Uniform { .. } => panic!("expected normal perturbation default"),
        }
        match RandomGuessConfig::default().random.distribution {
            DistributionConfig::Uniform { config } => {
                assert_eq!(config.min, -1.0);
                assert_eq!(config.max, 1.0);
            }
            DistributionConfig::Normal { .. } => panic!("expected uniform random guess default"),
        }
    }

    #[test]
    fn test_hf_config_one_electron_guess_perturbation_deserialization() {
        let config: HfConfig = toml_spanner::from_str(
            r#"
            [guess]
            type = "OneElectron"

            [guess.perturbation]
            distribution = "Uniform"
            min = -1e-4
            max = 1e-4
            seed = 43
            "#,
        )
        .unwrap();

        let DensityGuessConfig::OneElectron {
            perturbation: Some(perturbation),
        } = config.guess
        else {
            panic!("expected perturbed one electron guess config");
        };
        assert_eq!(perturbation.random.seed, Some(43));
    }

    #[test]
    fn test_hf_config_serializes_random_config_only_for_random_guess() {
        let core_config: HfConfig = toml_spanner::from_str(
            r#"
            [guess]
            type = "CoreHamiltonian"
            "#,
        )
        .unwrap();
        let core_toml = toml_spanner::to_string(&core_config).unwrap();

        assert!(core_toml.contains("type = \"CoreHamiltonian\""));
        assert!(!core_toml.contains("perturbation"));
        assert!(!core_toml.contains("distribution"));
        assert!(!core_toml.contains("min"));
        assert!(!core_toml.contains("max ="));

        let random_config: HfConfig = toml_spanner::from_str(
            r#"
            [guess]
            type = "RandomSymmetric"
            distribution = "Normal"
            mean = 0.0
            std_dev = 0.5
            seed = 42
            "#,
        )
        .unwrap();
        let random_toml = toml_spanner::to_string(&random_config).unwrap();

        assert!(random_toml.contains("type = \"RandomSymmetric\""));
        assert!(random_toml.contains("distribution = \"Normal\""));
        assert!(random_toml.contains("seed = 42"));
    }

    #[test]
    fn test_hf_config_rejects_zero_max_iterations() {
        let result = toml_spanner::from_str::<HfConfig>("max_iterations = 0");

        assert!(result.is_err());
    }

    #[test]
    fn test_hf_config_rejects_non_positive_convergence_threshold() {
        let zero = toml_spanner::from_str::<HfConfig>("convergence_threshold = 0.0");
        let negative = toml_spanner::from_str::<HfConfig>("convergence_threshold = -1e-8");

        assert!(zero.is_err());
        assert!(negative.is_err());
    }

    #[test]
    fn test_hf_config_rejects_too_small_diis_size() {
        let result = toml_spanner::from_str::<HfConfig>(
            r#"
            diis = true
            diis_size = 1
            "#,
        );

        assert!(result.is_err());
    }
}
