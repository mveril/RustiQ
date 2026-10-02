//! Explicit normalized-request V1 wire records, independent of frontend syntax.
use std::num::{NonZeroU8, NonZeroUsize};

use serde::{Deserialize, Serialize};

use super::{calculation::Guess, PortableError};
use crate::{
    calculation::CalculationRequest,
    config::{
        validated::{DiisSize, NonNegativeFiniteF64, PositiveFiniteF64},
        HfConfig, HfMethod, MemoryLimit, MoleculeConfig, Mp2Config,
    },
    molecules::{atom::Atom, geometry::Geometry, units::Units},
};

pub(crate) const REQUEST_PATH: &str = "request.json";
pub(crate) const MAX_REQUEST_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequestSnapshot {
    format: String,
    version: u32,
    units: CoordinateUnits,
    atoms: Vec<RequestAtom>,
    charge: i32,
    multiplicity: u8,
    basis: String,
    hf: RequestedHf,
    mp2: Option<RequestedMp2>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum CoordinateUnits {
    Bohr,
    Angstrom,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestAtom {
    atomic_number: u32,
    position: [f64; 3],
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RequestedMethod {
    Auto,
    Rhf,
    Uhf,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestedHf {
    method: RequestedMethod,
    max_iterations: usize,
    convergence_threshold: f64,
    linear_dependency_threshold: f64,
    eri_schwarz_threshold: Option<f64>,
    diis: bool,
    diis_size: usize,
    guess: Guess,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestedMp2 {
    frozen_orbitals: usize,
    memory_limit: RequestedMemory,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum RequestedMemory {
    Auto,
    Fixed { bytes: u64 },
}

impl RequestSnapshot {
    pub(crate) fn from_request(request: &CalculationRequest) -> Self {
        let hf = request.hf();
        Self {
            format: "rustiq-request".into(),
            version: 1,
            units: match request.molecule().units {
                Units::Bohr => CoordinateUnits::Bohr,
                Units::Angstrom => CoordinateUnits::Angstrom,
            },
            atoms: request
                .geometry()
                .atoms
                .iter()
                .map(|atom| RequestAtom {
                    atomic_number: atom.element.atomic_number,
                    position: atom.position.into(),
                })
                .collect(),
            charge: request.molecule().charge.value,
            multiplicity: request.molecule().multiplicity.value.get(),
            basis: request.basis_name().into(),
            hf: RequestedHf {
                method: match hf.method.value {
                    HfMethod::Auto => RequestedMethod::Auto,
                    HfMethod::Rhf => RequestedMethod::Rhf,
                    HfMethod::Uhf => RequestedMethod::Uhf,
                },
                max_iterations: hf.max_iterations.get(),
                convergence_threshold: hf.convergence_threshold.into_inner(),
                linear_dependency_threshold: hf.linear_dependency_threshold.value.into_inner(),
                eri_schwarz_threshold: hf.eri_schwarz_threshold.map(|v| v.into_inner()),
                diis: hf.diis,
                diis_size: hf.diis_size.into_inner(),
                guess: hf.guess.value.into(),
            },
            mp2: request.mp2().map(|mp2| RequestedMp2 {
                frozen_orbitals: mp2.frozen_orbitals.value,
                memory_limit: match mp2.memory_limit.value {
                    MemoryLimit::Auto => RequestedMemory::Auto,
                    MemoryLimit::Fixed(bytes) => RequestedMemory::Fixed {
                        bytes: bytes.as_u64(),
                    },
                },
            }),
        }
    }

    pub(crate) fn to_request(&self) -> Result<CalculationRequest, PortableError> {
        if self.format != "rustiq-request" || self.version != 1 {
            return Err(PortableError::UnsupportedVersion);
        }
        let invalid = || PortableError::InvalidRequest("invalid normalized request".into());
        if self.atoms.is_empty() {
            return Err(invalid());
        }
        let atoms = self
            .atoms
            .iter()
            .map(|atom| {
                let element = periodic_table::periodic_table()
                    .iter()
                    .find(|e| e.atomic_number == atom.atomic_number)
                    .copied()
                    .ok_or_else(invalid)?;
                if !atom.position.iter().all(|v| v.is_finite()) {
                    return Err(invalid());
                }
                Ok(Atom::new(element, atom.position.into()))
            })
            .collect::<Result<Vec<_>, PortableError>>()?;
        self.hf
            .guess
            .validate()
            .map_err(|error| PortableError::InvalidRequest(error.to_string()))?;
        let hf = HfConfig {
            method: match self.hf.method {
                RequestedMethod::Auto => HfMethod::Auto,
                RequestedMethod::Rhf => HfMethod::Rhf,
                RequestedMethod::Uhf => HfMethod::Uhf,
            }
            .into(),
            max_iterations: NonZeroUsize::new(self.hf.max_iterations).ok_or_else(invalid)?,
            convergence_threshold: PositiveFiniteF64::try_new(self.hf.convergence_threshold)
                .map_err(|_| invalid())?,
            linear_dependency_threshold: NonNegativeFiniteF64::try_new(
                self.hf.linear_dependency_threshold,
            )
            .map_err(|_| invalid())?
            .into(),
            eri_schwarz_threshold: self
                .hf
                .eri_schwarz_threshold
                .map(PositiveFiniteF64::try_new)
                .transpose()
                .map_err(|_| invalid())?,
            guess: self.hf.guess.to_config().into(),
            diis: self.hf.diis,
            diis_size: DiisSize::try_new(self.hf.diis_size).map_err(|_| invalid())?,
        };
        let request = CalculationRequest {
            geometry: Geometry::new(String::new(), atoms),
            molecule: MoleculeConfig {
                units: match self.units {
                    CoordinateUnits::Bohr => Units::Bohr,
                    CoordinateUnits::Angstrom => Units::Angstrom,
                },
                charge: self.charge.into(),
                multiplicity: NonZeroU8::new(self.multiplicity)
                    .ok_or_else(invalid)?
                    .into(),
            },
            basis_name: self.basis.clone(),
            hf,
            mp2: self.mp2.as_ref().map(|mp2| Mp2Config {
                frozen_orbitals: mp2.frozen_orbitals.into(),
                memory_limit: match mp2.memory_limit {
                    RequestedMemory::Auto => MemoryLimit::Auto,
                    RequestedMemory::Fixed { bytes } => {
                        MemoryLimit::Fixed(bytesize::ByteSize(bytes))
                    }
                }
                .into(),
            }),
        };
        let molecule = request
            .molecule()
            .build(request.geometry().clone())
            .map_err(|error| PortableError::InvalidRequest(error.to_string()))?;
        request
            .hf()
            .resolve_method(&molecule)
            .map_err(|error| PortableError::InvalidRequest(error.to_string()))?;
        Ok(request)
    }
}
