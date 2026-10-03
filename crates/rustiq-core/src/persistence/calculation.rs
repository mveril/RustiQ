//! Explicit V1 wire records; domain structs are never serialized as the format.
use serde::{Deserialize, Serialize};

use super::{
    identity::CanonicalBytes, PortableError, ScientificIdentity, AO_ERI_COMPUTATION_VERSION,
};
use crate::{
    calculation::PreparedCalculation,
    config::{
        random_config::{DistributionConfig, RandomConfig},
        DensityGuessConfig, HfMethod, ResolvedHfMethod,
    },
    molecules::molecule::Molecule,
};

pub(crate) const CALCULATION_PATH: &str = "calculation.json";
pub(crate) const MAX_CALCULATION_BYTES: u64 = 64 * 1024 * 1024;

/// Read-only, resolved scientific context. Coordinates and centers are in Bohr.
#[derive(Clone, Debug)]
pub struct CalculationContext(pub(crate) Snapshot, DomainCounts);

// Checked once at the wire boundary; public accessors retain their domain types.
#[derive(Clone, Debug)]
struct DomainCounts {
    max_iterations: usize,
    diis_size: Option<usize>,
    frozen_orbitals: Option<usize>,
}
impl DomainCounts {
    fn from_snapshot(snapshot: &Snapshot) -> Result<Self, PortableError> {
        let invalid =
            |_| PortableError::InvalidCalculation("integer exceeds host usize range".into());
        Ok(Self {
            max_iterations: usize::try_from(snapshot.hf.max_iterations).map_err(invalid)?,
            diis_size: snapshot
                .hf
                .diis_size
                .map(usize::try_from)
                .transpose()
                .map_err(invalid)?,
            frozen_orbitals: snapshot
                .mp2
                .as_ref()
                .map(|v| usize::try_from(v.frozen_orbitals))
                .transpose()
                .map_err(invalid)?,
        })
    }
}

impl CalculationContext {
    pub(crate) fn from_snapshot(snapshot: Snapshot) -> Result<Self, PortableError> {
        let counts = DomainCounts::from_snapshot(&snapshot)?;
        Ok(Self(snapshot, counts))
    }
    pub fn atoms(&self) -> impl ExactSizeIterator<Item = (u32, [f64; 3])> + '_ {
        self.0
            .atoms
            .iter()
            .map(|atom| (atom.atomic_number, atom.position))
    }
    pub fn charge(&self) -> i32 {
        self.0.charge
    }
    pub fn multiplicity(&self) -> u8 {
        self.0.multiplicity
    }
    pub fn basis(&self) -> impl ExactSizeIterator<Item = ResolvedAo<'_>> {
        self.0.basis.iter().map(ResolvedAo)
    }
    pub fn hf_method(&self) -> ResolvedHfMethod {
        match self.0.hf.method {
            Method::Rhf => ResolvedHfMethod::Rhf,
            Method::Uhf => ResolvedHfMethod::Uhf,
        }
    }
    pub fn max_iterations(&self) -> usize {
        self.1.max_iterations
    }
    pub fn convergence_threshold(&self) -> f64 {
        self.0.hf.convergence_threshold
    }
    pub fn linear_dependency_threshold(&self) -> f64 {
        self.0.hf.linear_dependency_threshold
    }
    pub fn eri_schwarz_threshold(&self) -> Option<f64> {
        self.0.hf.eri_schwarz_threshold
    }
    pub fn diis_size(&self) -> Option<usize> {
        self.1.diis_size
    }
    /// `None` means HF only; `Some(n)` requests MP2 with n frozen orbitals.
    pub fn mp2_frozen_orbitals(&self) -> Option<usize> {
        self.1.frozen_orbitals
    }
    /// The initial density strategy; random settings do not promise an exact restart.
    pub fn density_guess(&self) -> crate::config::DensityGuessConfig {
        self.0.hf.guess.to_config()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Snapshot {
    format: String,
    version: u32,
    units: String,
    atoms: Vec<Atom>,
    charge: i32,
    multiplicity: u8,
    basis: Vec<Ao>,
    hf: Hf,
    mp2: Option<Mp2>,
    ao_eri_computation_version: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Atom {
    atomic_number: u32,
    position: [f64; 3],
}

/// One effective AO, in the same order as the integral engine.
#[derive(Clone, Copy, Debug)]
pub struct ResolvedAo<'a>(&'a Ao);
impl<'a> ResolvedAo<'a> {
    pub fn center(&self) -> [f64; 3] {
        self.0.center
    }
    pub fn components(&self) -> impl ExactSizeIterator<Item = ResolvedComponent<'a>> {
        self.0.components.iter().map(ResolvedComponent)
    }
}
/// A Cartesian component with effective normalized primitive coefficients.
#[derive(Clone, Copy, Debug)]
pub struct ResolvedComponent<'a>(&'a Component);
impl ResolvedComponent<'_> {
    pub fn angular_momentum(&self) -> [u8; 3] {
        self.0.angular_momentum
    }
    /// Returns (exponent, effective normalized coefficient) pairs.
    pub fn primitives(&self) -> impl ExactSizeIterator<Item = (f64, f64)> + '_ {
        self.0
            .primitives
            .iter()
            .map(|p| (p.exponent, p.coefficient))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ao {
    center: [f64; 3],
    components: Vec<Component>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Component {
    angular_momentum: [u8; 3],
    primitives: Vec<Primitive>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Primitive {
    exponent: f64,
    coefficient: f64,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Method {
    Rhf,
    Uhf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Hf {
    method: Method,
    max_iterations: u64,
    convergence_threshold: f64,
    linear_dependency_threshold: f64,
    eri_schwarz_threshold: Option<f64>,
    diis_size: Option<u64>,
    guess: ResolvedGuess,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Mp2 {
    frozen_orbitals: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Guess {
    CoreHamiltonian { perturbation: Option<Random> },
    OneElectron { perturbation: Option<Random> },
    Random { random: Random },
    Zero,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Random {
    seed: Option<u64>,
    distribution: Distribution,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ResolvedGuess {
    CoreHamiltonian {
        perturbation: Option<ResolvedRandom>,
    },
    OneElectron {
        perturbation: Option<ResolvedRandom>,
    },
    Random { random: ResolvedRandom },
    Zero,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolvedRandom {
    seed: u64,
    distribution: Distribution,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Distribution {
    Uniform { min: f64, max: f64 },
    Normal { mean: f64, std_dev: f64 },
}

impl Snapshot {
    pub(crate) fn from_calculation(
        calculation: &PreparedCalculation,
    ) -> Result<Self, PortableError> {
        let invalid = |_| PortableError::InvalidCalculation("integer exceeds V1 u64 range".into());
        let molecule = calculation.get_molecule();
        let basis = calculation.get_basis();
        let hf = calculation.hf_config();
        let method = calculation.hf_method();
        let mp2 = calculation.mp2_config();
        let snapshot = Self {
            format: "rustiq-calculation".into(),
            version: 1,
            units: "bohr".into(),
            atoms: molecule
                .atoms
                .iter()
                .map(|atom| Atom {
                    atomic_number: atom.element.atomic_number,
                    position: atom.position.into(),
                })
                .collect(),
            charge: molecule.charge(),
            multiplicity: molecule.multiplicity().get(),
            basis: basis
                .shell_ids
                .iter()
                .zip(&basis.normalized_components)
                .map(|(shell, components)| Ao {
                    center: basis.shells[*shell].origin.into(),
                    components: components
                        .iter()
                        .map(|component| Component {
                            angular_momentum: component.angular_momentum.into(),
                            primitives: component
                                .primitives
                                .iter()
                                .map(|p| Primitive {
                                    exponent: p.exponent,
                                    coefficient: p.coefficient,
                                })
                                .collect(),
                        })
                        .collect(),
                })
                .collect(),
            hf: Hf {
                method: match method {
                    ResolvedHfMethod::Rhf => Method::Rhf,
                    ResolvedHfMethod::Uhf => Method::Uhf,
                },
                max_iterations: u64::try_from(hf.max_iterations.get()).map_err(invalid)?,
                convergence_threshold: hf.convergence_threshold.into_inner(),
                linear_dependency_threshold: hf.linear_dependency_threshold.value.into_inner(),
                eri_schwarz_threshold: hf.eri_schwarz_threshold.map(|value| value.into_inner()),
                diis_size: hf
                    .diis
                    .then(|| u64::try_from(hf.diis_size.into_inner()))
                    .transpose()
                    .map_err(invalid)?,
                guess: ResolvedGuess::from_config(hf.guess.value)?,
            },
            mp2: mp2
                .map(|mp2| {
                    Ok::<_, PortableError>(Mp2 {
                        frozen_orbitals: u64::try_from(mp2.frozen_orbitals.value)
                            .map_err(invalid)?,
                    })
                })
                .transpose()?,
            ao_eri_computation_version: AO_ERI_COMPUTATION_VERSION,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub(crate) fn validate(&self) -> Result<(), PortableError> {
        if self.format != "rustiq-calculation" || self.version != 1 || self.units != "bohr" {
            return Err(PortableError::UnsupportedVersion);
        }
        let invalid = |message: &str| PortableError::InvalidCalculation(message.into());
        if self.atoms.is_empty()
            || self.basis.is_empty()
            || self.multiplicity == 0
            || self.ao_eri_computation_version == 0
        {
            return Err(invalid(
                "empty molecule/basis or invalid version/multiplicity",
            ));
        }
        let mut nuclear_charge = 0_i64;
        for atom in &self.atoms {
            if !(1..=118).contains(&atom.atomic_number)
                || !atom.position.iter().all(|v| v.is_finite())
            {
                return Err(invalid("invalid atom"));
            }
            nuclear_charge += i64::from(atom.atomic_number);
        }
        let electrons = nuclear_charge - i64::from(self.charge);
        let unpaired = i64::from(self.multiplicity) - 1;
        if electrons <= 0
            || electrons < unpaired
            || (electrons - unpaired) % 2 != 0
            || (matches!(self.hf.method, Method::Rhf) && (unpaired != 0 || electrons % 2 != 0))
        {
            return Err(invalid(
                "inconsistent electron count, multiplicity or HF method",
            ));
        }
        for ao in &self.basis {
            if !ao.center.iter().all(|v| v.is_finite()) || ao.components.is_empty() {
                return Err(invalid("invalid AO center or empty components"));
            }
            for component in &ao.components {
                if component.primitives.is_empty()
                    || component
                        .primitives
                        .iter()
                        .any(|p| !positive(p.exponent) || !p.coefficient.is_finite())
                {
                    return Err(invalid("invalid AO primitives"));
                }
            }
        }
        if crate::eri::CompactEri::checked_storage_len(self.basis.len())
            .and_then(|n| n.checked_mul(8))
            .is_none()
        {
            return Err(invalid("basis dimensions overflow"));
        }
        if self.hf.max_iterations == 0
            || !positive(self.hf.convergence_threshold)
            || !self.hf.linear_dependency_threshold.is_finite()
            || self.hf.linear_dependency_threshold < 0.0
            || self.hf.eri_schwarz_threshold.is_some_and(|v| !positive(v))
            || self.hf.diis_size.is_some_and(|v| v < 2)
        {
            return Err(invalid("invalid HF settings"));
        }
        DomainCounts::from_snapshot(self)?;
        self.hf.guess.validate()?;
        Ok(())
    }

    pub(crate) fn validate_request(
        &self,
        request: &crate::calculation::CalculationRequest,
    ) -> Result<(), PortableError> {
        let invalid =
            || PortableError::InvalidRequest("request disagrees with resolved calculation".into());
        let molecule = request
            .molecule()
            .build(request.geometry().clone())
            .map_err(|_| invalid())?;
        let method =
            resolve_hf_method_v1(request.hf().method.value, &molecule).ok_or_else(invalid)?;
        let hf = request.hf();
        if molecule.charge() != self.charge
            || molecule.multiplicity().get() != self.multiplicity
            || molecule.atoms.len() != self.atoms.len()
            || molecule.atoms.iter().zip(&self.atoms).any(|(a, b)| {
                a.element.atomic_number != b.atomic_number
                    || !<[f64; 3]>::from(a.position)
                        .into_iter()
                        .zip(b.position)
                        .all(|(requested, resolved)| {
                            coordinate_matches_v1(requested, resolved, request.molecule().units)
                        })
            })
            || !matches!(
                (method, self.hf.method),
                (ResolvedHfMethod::Rhf, Method::Rhf) | (ResolvedHfMethod::Uhf, Method::Uhf)
            )
            || u64::try_from(hf.max_iterations.get()).map_err(|_| invalid())?
                != self.hf.max_iterations
            || hf.convergence_threshold.into_inner() != self.hf.convergence_threshold
            || hf.linear_dependency_threshold.value.into_inner()
                != self.hf.linear_dependency_threshold
            || hf.eri_schwarz_threshold.map(|v| v.into_inner()) != self.hf.eri_schwarz_threshold
            || hf
                .diis
                .then(|| u64::try_from(hf.diis_size.into_inner()))
                .transpose()
                .map_err(|_| invalid())?
                != self.hf.diis_size
            || request
                .mp2()
                .map(|v| u64::try_from(v.frozen_orbitals.value))
                .transpose()
                .map_err(|_| invalid())?
                != self.mp2.as_ref().map(|v| v.frozen_orbitals)
            || !self.hf.guess.is_resolution_of(&hf.guess.value.into())
        {
            return Err(invalid());
        }
        Ok(())
    }

    pub(crate) fn basis_functions(&self) -> usize {
        self.basis.len()
    }
    pub(crate) fn computation_version(&self) -> u32 {
        self.ao_eri_computation_version
    }
    pub(crate) fn identity(&self) -> ScientificIdentity {
        let mut bytes = CanonicalBytes::ao_eri(self.ao_eri_computation_version);
        bytes.len(self.atoms.len());
        for atom in &self.atoms {
            bytes.u32(atom.atomic_number);
            for coordinate in atom.position {
                bytes.f64(coordinate);
            }
        }
        bytes.len(self.basis.len());
        for ao in &self.basis {
            for coordinate in ao.center {
                bytes.f64(coordinate);
            }
            bytes.len(ao.components.len());
            for component in &ao.components {
                for axis in component.angular_momentum {
                    bytes.u8(axis);
                }
                bytes.len(component.primitives.len());
                for primitive in &component.primitives {
                    bytes.f64(primitive.exponent);
                    bytes.f64(primitive.coefficient);
                }
            }
        }
        bytes.finish_ao_eri(self.hf.eri_schwarz_threshold)
    }
}
/// Resolves the HF request according to the frozen V1 persistence contract.
///
/// V1 predates any future method that may become eligible for `Auto`, so archive
/// validation must not inherit changes to the current domain-level resolver.
pub(super) fn resolve_hf_method_v1(
    method: HfMethod,
    molecule: &Molecule,
) -> Option<ResolvedHfMethod> {
    let closed_shell_singlet =
        molecule.multiplicity().get() == 1 && molecule.total_electrons().is_multiple_of(2);
    match method {
        HfMethod::Rhf if closed_shell_singlet => Some(ResolvedHfMethod::Rhf),
        HfMethod::Rhf => None,
        HfMethod::Uhf => Some(ResolvedHfMethod::Uhf),
        HfMethod::Auto if closed_shell_singlet => Some(ResolvedHfMethod::Rhf),
        HfMethod::Auto => Some(ResolvedHfMethod::Uhf),
    }
}

// V1 pins this decimal value (CODATA 2018 Bohr radius in Angstrom), rather
// than inheriting a future physical_constants release. Parse as binary64 and divide.
const V1_BOHR_IN_ANGSTROM: f64 = 0.529_177_210_903;

fn coordinate_matches_v1(
    requested: f64,
    resolved: f64,
    units: crate::molecules::units::Units,
) -> bool {
    use crate::molecules::units::Units;
    let expected = match units {
        Units::Bohr => requested,
        Units::Angstrom => requested / V1_BOHR_IN_ANGSTROM,
    };
    if !expected.is_finite() || !resolved.is_finite() {
        return false;
    }
    if units == Units::Bohr {
        return expected == resolved;
    }
    // Four adjacent binary64 values cover division versus reciprocal multiplication
    // and final rounding of independently produced conversions. There is no absolute
    // chemistry-level floor: even near zero only four subnormal steps are allowed.
    fn ordered(value: f64) -> u64 {
        let bits = if value == 0.0 { 0 } else { value.to_bits() };
        if bits >> 63 == 0 {
            bits | (1 << 63)
        } else {
            (!bits) + 1
        }
    }
    ordered(expected).abs_diff(ordered(resolved)) <= 4
}

fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}
impl From<RandomConfig> for Random {
    fn from(config: RandomConfig) -> Self {
        Self {
            seed: config.seed,
            distribution: match config.distribution {
                DistributionConfig::Uniform { config } => Distribution::Uniform {
                    min: config.min,
                    max: config.max,
                },
                DistributionConfig::Normal { config } => Distribution::Normal {
                    mean: config.mean,
                    std_dev: config.std_dev.into_inner(),
                },
            },
        }
    }
}
impl Random {
    fn to_config(&self) -> RandomConfig {
        use crate::config::random_config::distribution_config::{
            NormalDistributionConfig, UniformDistributionConfig,
        };
        RandomConfig {
            seed: self.seed,
            distribution: match self.distribution {
                Distribution::Uniform { min, max } => DistributionConfig::Uniform {
                    config: UniformDistributionConfig { min, max },
                },
                Distribution::Normal { mean, std_dev } => DistributionConfig::Normal {
                    config: NormalDistributionConfig {
                        mean,
                        std_dev: crate::config::validated::PositiveFiniteF64::try_new(std_dev)
                            .expect("validated snapshot"),
                    },
                },
            },
        }
    }
}
impl ResolvedRandom {
    fn from_config(config: RandomConfig) -> Result<Self, PortableError> {
        let requested = Random::from(config);
        let seed = requested.seed.ok_or_else(|| {
            PortableError::InvalidCalculation(
                "resolved random configuration is missing its seed".into(),
            )
        })?;
        Ok(Self {
            seed,
            distribution: requested.distribution,
        })
    }

    fn to_config(&self) -> RandomConfig {
        Random {
            seed: Some(self.seed),
            distribution: self.distribution.clone(),
        }
        .to_config()
    }
}

impl ResolvedGuess {
    fn from_config(config: DensityGuessConfig) -> Result<Self, PortableError> {
        Ok(match config {
            DensityGuessConfig::CoreHamiltonian { perturbation } => Self::CoreHamiltonian {
                perturbation: perturbation
                    .map(|value| ResolvedRandom::from_config(value.random))
                    .transpose()?,
            },
            DensityGuessConfig::OneElectron { perturbation } => Self::OneElectron {
                perturbation: perturbation
                    .map(|value| ResolvedRandom::from_config(value.random))
                    .transpose()?,
            },
            DensityGuessConfig::Random { config } => Self::Random {
                random: ResolvedRandom::from_config(config.random)?,
            },
            DensityGuessConfig::Zero => Self::Zero,
        })
    }

    fn is_resolution_of(&self, requested: &Guess) -> bool {
        let random_matches = |resolved: &ResolvedRandom, requested: &Random| {
            resolved.distribution == requested.distribution
                && requested.seed.is_none_or(|seed| resolved.seed == seed)
        };
        match (self, requested) {
            (
                Self::CoreHamiltonian { perturbation: a },
                Guess::CoreHamiltonian { perturbation: b },
            )
            | (
                Self::OneElectron { perturbation: a },
                Guess::OneElectron { perturbation: b },
            ) => match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => random_matches(a, b),
                _ => false,
            },
            (Self::Random { random: a }, Guess::Random { random: b }) => random_matches(a, b),
            (Self::Zero, Guess::Zero) => true,
            _ => false,
        }
    }

    fn validate(&self) -> Result<(), PortableError> {
        let random = match self {
            Self::CoreHamiltonian { perturbation } | Self::OneElectron { perturbation } => {
                perturbation.as_ref()
            }
            Self::Random { random } => Some(random),
            Self::Zero => None,
        };
        if let Some(random) = random {
            let valid = match random.distribution {
                Distribution::Uniform { min, max } => {
                    min.is_finite() && max.is_finite() && min < max && (max - min).is_finite()
                }
                Distribution::Normal { mean, std_dev } => mean.is_finite() && positive(std_dev),
            };
            if !valid {
                return Err(PortableError::InvalidCalculation(
                    "invalid random distribution".into(),
                ));
            }
        }
        Ok(())
    }

    fn to_config(&self) -> DensityGuessConfig {
        use crate::config::{GuessPerturbationConfig, RandomGuessConfig};
        match self {
            Self::CoreHamiltonian { perturbation } => DensityGuessConfig::CoreHamiltonian {
                perturbation: perturbation.as_ref().map(|value| GuessPerturbationConfig {
                    random: value.to_config(),
                }),
            },
            Self::OneElectron { perturbation } => DensityGuessConfig::OneElectron {
                perturbation: perturbation.as_ref().map(|value| GuessPerturbationConfig {
                    random: value.to_config(),
                }),
            },
            Self::Random { random } => DensityGuessConfig::Random {
                config: RandomGuessConfig {
                    random: random.to_config(),
                },
            },
            Self::Zero => DensityGuessConfig::Zero,
        }
    }
}

impl Guess {
    pub(super) fn validate(&self) -> Result<(), PortableError> {
        let random = match self {
            Guess::CoreHamiltonian { perturbation } | Guess::OneElectron { perturbation } => {
                perturbation.as_ref()
            }
            Guess::Random { random } => Some(random),
            Guess::Zero => None,
        };
        if let Some(random) = random {
            let valid = match random.distribution {
                Distribution::Uniform { min, max } => {
                    min.is_finite() && max.is_finite() && min < max && (max - min).is_finite()
                }
                Distribution::Normal { mean, std_dev } => mean.is_finite() && positive(std_dev),
            };
            if !valid {
                return Err(PortableError::InvalidCalculation(
                    "invalid random distribution".into(),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn to_config(&self) -> DensityGuessConfig {
        use crate::config::{GuessPerturbationConfig, RandomGuessConfig};
        match self {
            Self::CoreHamiltonian { perturbation } => DensityGuessConfig::CoreHamiltonian {
                perturbation: perturbation.as_ref().map(|p| GuessPerturbationConfig {
                    random: p.to_config(),
                }),
            },
            Self::OneElectron { perturbation } => DensityGuessConfig::OneElectron {
                perturbation: perturbation.as_ref().map(|p| GuessPerturbationConfig {
                    random: p.to_config(),
                }),
            },
            Self::Random { random } => DensityGuessConfig::Random {
                config: RandomGuessConfig {
                    random: random.to_config(),
                },
            },
            Self::Zero => DensityGuessConfig::Zero,
        }
    }
}

impl From<DensityGuessConfig> for Guess {
    fn from(config: DensityGuessConfig) -> Self {
        match config {
            DensityGuessConfig::CoreHamiltonian { perturbation } => Guess::CoreHamiltonian {
                perturbation: perturbation.map(|p| p.random.into()),
            },
            DensityGuessConfig::OneElectron { perturbation } => Guess::OneElectron {
                perturbation: perturbation.map(|p| p.random.into()),
            },
            DensityGuessConfig::Random { config } => Guess::Random {
                random: config.random.into(),
            },
            DensityGuessConfig::Zero => Guess::Zero,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn json_round_trip_preserves_binary64_scientific_identity(
            coordinate in any::<f64>().prop_filter("finite coordinate", |v| v.is_finite()),
            exponent in any::<f64>().prop_filter("positive finite exponent", |v| v.is_finite() && *v > 0.0),
            coefficient in any::<f64>().prop_filter("finite coefficient", |v| v.is_finite()),
        ) {
            let mut snapshot: Snapshot = serde_json::from_str(include_str!("../../tests/data/persistence/calculation-h2-v1.json")).unwrap();
            snapshot.atoms[0].position[0] = coordinate;
            snapshot.basis[0].center[0] = coordinate;
            snapshot.basis[0].components[0].primitives[0].exponent = exponent;
            snapshot.basis[0].components[0].primitives[0].coefficient = coefficient;
            snapshot.validate().unwrap();
            let bytes = serde_json::to_vec(&snapshot).unwrap();
            let restored: Snapshot = serde_json::from_slice(&bytes).unwrap();
            restored.validate().unwrap();
            prop_assert_eq!(snapshot.identity(), restored.identity());
        }
    }
}

#[cfg(test)]
mod v1_contract_tests {
    use super::*;
    use crate::molecules::{atom::Atom, geometry::Geometry, units::Units};
    use nalgebra::point;
    use serde_json::{json, Value};
    use std::num::NonZeroU8;

    fn molecule(symbols: &[&str], multiplicity: u8) -> Molecule {
        let elements = periodic_table::periodic_table();
        let atoms = symbols
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
        Molecule::try_new(
            Geometry::new("v1-method-resolution".into(), atoms),
            Units::Bohr,
            0,
            NonZeroU8::new(multiplicity).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn hf_method_resolution_is_frozen_for_v1() {
        let closed_shell = molecule(&["H", "H"], 1);
        let open_shell = molecule(&["H"], 2);

        assert_eq!(
            resolve_hf_method_v1(HfMethod::Auto, &closed_shell),
            Some(ResolvedHfMethod::Rhf)
        );
        assert_eq!(
            resolve_hf_method_v1(HfMethod::Auto, &open_shell),
            Some(ResolvedHfMethod::Uhf)
        );
        assert_eq!(
            resolve_hf_method_v1(HfMethod::Rhf, &closed_shell),
            Some(ResolvedHfMethod::Rhf)
        );
        assert_eq!(resolve_hf_method_v1(HfMethod::Rhf, &open_shell), None);
        assert_eq!(
            resolve_hf_method_v1(HfMethod::Uhf, &closed_shell),
            Some(ResolvedHfMethod::Uhf)
        );
        assert_eq!(
            resolve_hf_method_v1(HfMethod::Uhf, &open_shell),
            Some(ResolvedHfMethod::Uhf)
        );
    }

    #[test]
    fn resolved_random_seed_is_required_by_v1() {
        let schema: Value = serde_json::from_str(include_str!(
            "../../../../schemas/calculation-snapshot-v1.schema.json"
        ))
        .unwrap();
        assert!(schema["$defs"]["random"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("seed")));
        assert_eq!(
            schema["$defs"]["random"]["properties"]["seed"]["type"],
            json!("integer")
        );

        let mut value: Value = serde_json::from_str(include_str!(
            "../../tests/data/persistence/calculation-h2-v1.json"
        ))
        .unwrap();
        value["hf"]["guess"] = json!({
            "kind": "random",
            "random": {
                "seed": 42,
                "distribution": {"kind": "uniform", "min": -1.0, "max": 1.0}
            }
        });
        let snapshot: Snapshot = serde_json::from_value(value.clone()).unwrap();
        snapshot.validate().unwrap();

        value["hf"]["guess"]["random"]
            .as_object_mut()
            .unwrap()
            .remove("seed");
        assert!(serde_json::from_value::<Snapshot>(value.clone()).is_err());

        value["hf"]["guess"]["random"]["seed"] = Value::Null;
        assert!(serde_json::from_value::<Snapshot>(value).is_err());
    }

    #[test]
    fn conversion_comparison_is_bounded_in_binary64_steps() {
        for requested in [0.74, -0.74, 1e-200, -1e-200, 1e200] {
            let expected = requested / V1_BOHR_IN_ANGSTROM;
            let four = f64::from_bits(expected.to_bits() + 4);
            let five = f64::from_bits(expected.to_bits() + 5);
            assert!(coordinate_matches_v1(requested, four, Units::Angstrom));
            assert!(!coordinate_matches_v1(requested, five, Units::Angstrom));
            assert!(!coordinate_matches_v1(requested, four, Units::Bohr));
        }
        assert!(coordinate_matches_v1(0.0, -0.0, Units::Angstrom));
        assert!(coordinate_matches_v1(
            0.0,
            f64::from_bits(4),
            Units::Angstrom
        ));
        assert!(!coordinate_matches_v1(
            0.0,
            f64::from_bits(5),
            Units::Angstrom
        ));
        assert!(!coordinate_matches_v1(0.0, 1e-15, Units::Angstrom));
        assert!(!coordinate_matches_v1(f64::MAX, f64::MAX, Units::Angstrom));
        assert!(!coordinate_matches_v1(0.74, f64::NAN, Units::Angstrom));
        assert!(!coordinate_matches_v1(0.74, f64::INFINITY, Units::Angstrom));
    }

    #[test]
    fn resolved_counts_use_u64_wire_and_checked_host_values() {
        let schema: Value = serde_json::from_str(include_str!(
            "../../../../schemas/calculation-snapshot-v1.schema.json"
        ))
        .unwrap();
        for (definition, field) in [
            ("hf", "max_iterations"),
            ("hf", "diis_size"),
            ("mp2", "frozen_orbitals"),
        ] {
            let property = &schema["$defs"][definition]["properties"][field];
            let integer = if field == "diis_size" {
                &property["anyOf"][0]
            } else {
                property
            };
            assert_eq!(integer["maximum"], json!(u64::MAX));
            let mut value: Value = serde_json::from_str(include_str!(
                "../../tests/data/persistence/calculation-h2-v1.json"
            ))
            .unwrap();
            value["mp2"] = json!({"frozen_orbitals":0});
            value[definition][field] = json!(u64::MAX);
            let snapshot: Snapshot = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(
                snapshot.validate().is_ok(),
                usize::try_from(u64::MAX).is_ok()
            );
            assert_eq!(
                CalculationContext::from_snapshot(snapshot.clone()).is_ok(),
                usize::try_from(u64::MAX).is_ok()
            );
            assert_eq!(
                serde_json::to_value(snapshot).unwrap()[definition][field],
                json!(u64::MAX)
            );
            for invalid in ["-1", "18446744073709551616"] {
                let encoded = serde_json::to_string(&value)
                    .unwrap()
                    .replace("18446744073709551615", invalid);
                assert!(serde_json::from_str::<Snapshot>(&encoded).is_err());
            }
        }
    }
}
