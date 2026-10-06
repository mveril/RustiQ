//! Explicit normalized-request V1 wire records, independent of frontend syntax.
use std::num::{NonZeroU8, NonZeroUsize};

use serde::{Deserialize, Serialize};

use super::{
    calculation::{resolve_hf_method_v1, Guess},
    PortableError,
};
use crate::{
    calculation::CalculationRequest,
    config::{
        validated::{DiisSize, NonNegativeFiniteF64, PositiveFiniteF64},
        DiisConfig, HfConfig, HfMethod, IntegralConfig, MemoryLimit, MoleculeConfig, Mp2Config,
        OrthogonalizationConfig,
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
    max_iterations: u64,
    convergence_threshold: f64,
    linear_dependency_threshold: f64,
    eri_schwarz_threshold: Option<f64>,
    diis: bool,
    diis_size: u64,
    guess: Guess,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestedMp2 {
    frozen_orbitals: u64,
    memory_limit: RequestedMemory,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum RequestedMemory {
    Auto,
    Fixed { bytes: u64 },
}

impl RequestSnapshot {
    pub(crate) fn from_request(request: &CalculationRequest) -> Result<Self, PortableError> {
        let invalid = || PortableError::InvalidRequest("integer exceeds V1 u64 range".into());
        let hf = request.hf();
        Ok(Self {
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
                max_iterations: u64::try_from(hf.max_iterations.get()).map_err(|_| invalid())?,
                convergence_threshold: hf.convergence_threshold.into_inner(),
                linear_dependency_threshold: hf
                    .orthogonalization
                    .linear_dependency_threshold
                    .value
                    .into_inner(),
                eri_schwarz_threshold: request
                    .integrals()
                    .schwarz_threshold
                    .value
                    .map(|v| v.into_inner()),
                diis: hf.diis.enabled,
                diis_size: u64::try_from(hf.diis.max_history.value.into_inner())
                    .map_err(|_| invalid())?,
                guess: hf.guess.value.into(),
            },
            mp2: request
                .mp2()
                .map(|mp2| {
                    Ok::<_, PortableError>(RequestedMp2 {
                        frozen_orbitals: u64::try_from(mp2.frozen_orbitals.value)
                            .map_err(|_| invalid())?,
                        memory_limit: match mp2.memory_limit.value {
                            MemoryLimit::Auto => RequestedMemory::Auto,
                            MemoryLimit::Fixed(bytes) => RequestedMemory::Fixed {
                                bytes: bytes.as_u64(),
                            },
                        },
                    })
                })
                .transpose()?,
        })
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
            max_iterations: NonZeroUsize::new(
                usize::try_from(self.hf.max_iterations).map_err(|_| invalid())?,
            )
            .ok_or_else(invalid)?,
            convergence_threshold: PositiveFiniteF64::try_new(self.hf.convergence_threshold)
                .map_err(|_| invalid())?,
            orthogonalization: OrthogonalizationConfig {
                linear_dependency_threshold: NonNegativeFiniteF64::try_new(
                    self.hf.linear_dependency_threshold,
                )
                .map_err(|_| invalid())?
                .into(),
            },
            guess: self.hf.guess.to_config().into(),
            diis: DiisConfig {
                enabled: self.hf.diis,
                max_history: DiisSize::try_new(
                    usize::try_from(self.hf.diis_size).map_err(|_| invalid())?,
                )
                .map_err(|_| invalid())?
                .into(),
            },
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
            integrals: IntegralConfig {
                schwarz_threshold: self
                    .hf
                    .eri_schwarz_threshold
                    .map(PositiveFiniteF64::try_new)
                    .transpose()
                    .map_err(|_| invalid())?
                    .into(),
            },
            mp2: self
                .mp2
                .as_ref()
                .map(|mp2| {
                    Ok::<_, PortableError>(Mp2Config {
                        frozen_orbitals: usize::try_from(mp2.frozen_orbitals)
                            .map_err(|_| invalid())?
                            .into(),
                        memory_limit: match mp2.memory_limit {
                            RequestedMemory::Auto => MemoryLimit::Auto,
                            RequestedMemory::Fixed { bytes } => {
                                MemoryLimit::Fixed(bytesize::ByteSize(bytes))
                            }
                        }
                        .into(),
                    })
                })
                .transpose()?,
        };
        let molecule = request
            .molecule()
            .build(request.geometry().clone())
            .map_err(|error| PortableError::InvalidRequest(error.to_string()))?;
        resolve_hf_method_v1(request.hf().method.value, &molecule).ok_or_else(|| {
            PortableError::InvalidRequest("invalid V1 HF method resolution".into())
        })?;
        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn golden() -> Value {
        serde_json::from_str(include_str!(
            "../../tests/data/persistence/request-h2-v1.json"
        ))
        .unwrap()
    }

    #[test]
    fn schema_and_decoder_require_diis_size_even_when_disabled() {
        let schema: Value = serde_json::from_str(include_str!(
            "../../../../schemas/request-snapshot-v1.schema.json"
        ))
        .unwrap();
        assert!(schema["$defs"]["hf"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("diis_size")));
        for enabled in [false, true] {
            let mut value = golden();
            value["hf"]["diis"] = json!(enabled);
            value["hf"]["diis_size"] = json!(17);
            let snapshot: RequestSnapshot = serde_json::from_value(value.clone()).unwrap();
            let request = snapshot.to_request().unwrap();
            assert_eq!(request.hf().diis.enabled, enabled);
            assert_eq!(request.hf().diis.max_history.value.into_inner(), 17);
            assert_eq!(
                serde_json::to_value(RequestSnapshot::from_request(&request).unwrap()).unwrap()
                    ["hf"]["diis_size"],
                json!(17)
            );
            value["hf"].as_object_mut().unwrap().remove("diis_size");
            assert!(serde_json::from_value::<RequestSnapshot>(value).is_err());
        }
    }

    #[test]
    fn unsigned_wire_limits_and_checked_domain_conversions() {
        let schema: Value = serde_json::from_str(include_str!(
            "../../../../schemas/request-snapshot-v1.schema.json"
        ))
        .unwrap();
        for (definition, field) in [
            ("hf", "max_iterations"),
            ("hf", "diis_size"),
            ("mp2", "frozen_orbitals"),
        ] {
            assert_eq!(
                schema["$defs"][definition]["properties"][field]["maximum"],
                json!(u64::MAX)
            );
            let mut value = golden();
            value["mp2"] = json!({"frozen_orbitals":0,"memory_limit":{"kind":"auto"}});
            value[definition][field] = json!(u64::MAX);
            let snapshot: RequestSnapshot = serde_json::from_value(value.clone()).unwrap();
            let decoded = snapshot.to_request();
            assert_eq!(decoded.is_ok(), usize::try_from(u64::MAX).is_ok());
            if let Ok(request) = decoded {
                assert_eq!(
                    serde_json::to_value(RequestSnapshot::from_request(&request).unwrap()).unwrap()
                        [definition][field],
                    json!(u64::MAX)
                );
            }
            for invalid in ["-1", "18446744073709551616"] {
                let encoded = serde_json::to_string(&value)
                    .unwrap()
                    .replace("18446744073709551615", invalid);
                assert!(serde_json::from_str::<RequestSnapshot>(&encoded).is_err());
            }
            // A 32-bit reader must reject this valid V1 integer, not truncate it.
            value[definition][field] = json!(u64::from(u32::MAX) + 1);
            let snapshot: RequestSnapshot = serde_json::from_value(value).unwrap();
            assert_eq!(snapshot.to_request().is_ok(), usize::BITS > 32);
        }
    }
}
