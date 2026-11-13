use std::sync::Arc;

use nalgebra::{DMatrix, DVector};
use relative_path::RelativePath;
use serde::{Deserialize, Serialize};

use super::{artifact::private, Artifact, RustiQData};
use crate::{
    calculation::{AnyHfSolution, PreparedCalculation},
    config::ResolvedHfMethod,
    persistence::{ArtifactError, Sha256Digest},
};

pub(super) const HF_PATH: &str = "orbitals/converged-hf.json";
pub(super) const HF_REPRESENTATION: &str = "rustiq-converged-hf-npy-v1";
const MAX_HF_BYTES: u64 = 256 * 1024 * 1024;

/// Converged HF orbitals, independently owned and cheap to clone. Contains no AO integrals.
/// This inspection artifact cannot be converted into an executable MP2 reference.
#[derive(Clone, Debug)]
pub struct ConvergedHf(Arc<HfRecord>);

/// One spatial-orbital channel. Coefficients are column-major, in original AO row order.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HfOrbitalChannel {
    coefficients: Vec<f64>,
    energies: Vec<f64>,
    occupied: u64,
}
impl HfOrbitalChannel {
    /// Borrows column-major coefficients; the parent artifact supplies the AO row count.
    #[must_use]
    pub fn coefficients(&self) -> &[f64] {
        &self.coefficients
    }
    /// Orbital energies in Hartree, in coefficient-column order.
    #[must_use]
    pub fn energies(&self) -> &[f64] {
        &self.energies
    }
    /// Number of occupied columns, forming a prefix of the channel.
    #[must_use]
    pub fn occupied(&self) -> u64 {
        self.occupied
    }
    fn from_solution(c: &DMatrix<f64>, e: &DVector<f64>, occupied: usize) -> Self {
        Self {
            coefficients: c.as_slice().to_vec(),
            energies: e.as_slice().to_vec(),
            occupied: occupied as u64,
        }
    }
}

/// Reference-specific orbital layout; RHF orbitals are doubly occupied, UHF singly occupied.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "reference", rename_all = "lowercase", deny_unknown_fields)]
pub enum HfOrbitals<C = HfOrbitalChannel> {
    Rhf { shared: C },
    Uhf { alpha: C, beta: C },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HfRecord<O = HfOrbitals> {
    version: u32,
    identity: Sha256Digest,
    basis_dimension: u64,
    effective_rank: u64,
    iterations: u64,
    electronic_energy: f64,
    nuclear_repulsion_energy: f64,
    total_energy: f64,
    delta_energy: f64,
    residual_norm: f64,
    linear_dependency_threshold: f64,
    // S^2, ideal S^2, and contamination, absent for RHF.
    spin: Option<[f64; 3]>,
    orbitals: O,
}

impl ConvergedHf {
    /// Copies orbital data from a converged solution, preserving its originating HF identity.
    /// AO integrals are neither copied nor retained.
    ///
    /// # Errors
    /// Returns an error if the solution lacks a portable scientific identity.
    pub fn from_solution(solution: &AnyHfSolution) -> Result<Self, ArtifactError> {
        match solution {
            AnyHfSolution::Restricted(hf) => hf.as_scf().to_artifact(),
            AnyHfSolution::Unrestricted(hf) => hf.as_scf().to_artifact(),
        }
    }

    fn from_runtime(solution: &AnyHfSolution) -> Result<Self, ArtifactError> {
        let s = &solution.summary().scf;
        let orbitals = match solution {
            AnyHfSolution::Restricted(hf) => HfOrbitals::Rhf {
                shared: HfOrbitalChannel::from_solution(
                    hf.mo_coefficients(),
                    hf.orbital_energies(),
                    *hf.occupied_orbitals(),
                ),
            },
            AnyHfSolution::Unrestricted(hf) => HfOrbitals::Uhf {
                alpha: HfOrbitalChannel::from_solution(
                    &hf.mo_coefficients().alpha,
                    &hf.orbital_energies().alpha,
                    hf.occupied_orbitals().alpha,
                ),
                beta: HfOrbitalChannel::from_solution(
                    &hf.mo_coefficients().beta,
                    &hf.orbital_energies().beta,
                    hf.occupied_orbitals().beta,
                ),
            },
        };
        Ok(Self(Arc::new(HfRecord {
            version: 1,
            identity: solution
                .summary()
                .persistence_identity
                .ok_or_else(|| invalid("solution has no portable HF identity"))?,
            basis_dimension: s.orthogonalization.basis_dimension as u64,
            effective_rank: s.orthogonalization.effective_rank as u64,
            iterations: s.iterations as u64,
            electronic_energy: s.electronic_energy,
            nuclear_repulsion_energy: s.nuclear_repulsion_energy,
            total_energy: s.total_energy,
            delta_energy: s.delta_energy,
            residual_norm: s.residual_norm,
            linear_dependency_threshold: s.orthogonalization.relative_threshold,
            spin: s
                .spin
                .map(|v| [v.s_squared, v.ideal_s_squared, v.spin_contamination]),
            orbitals,
        })))
    }
    /// HF context identity V1, distinct from the entry's AO ERI identity.
    #[must_use]
    pub fn identity(&self) -> Sha256Digest {
        self.0.identity
    }
    #[must_use]
    pub fn version(&self) -> u32 {
        self.0.version
    }
    #[must_use]
    pub fn linear_dependency_threshold(&self) -> f64 {
        self.0.linear_dependency_threshold
    }
    #[must_use]
    pub fn orbitals(&self) -> &HfOrbitals {
        &self.0.orbitals
    }
    #[must_use]
    pub fn basis_dimension(&self) -> u64 {
        self.0.basis_dimension
    }
    #[must_use]
    pub fn effective_rank(&self) -> u64 {
        self.0.effective_rank
    }
    /// Electronic, nuclear repulsion, and total HF energies, respectively, in Hartree.
    #[must_use]
    pub fn energies(&self) -> [f64; 3] {
        [
            self.0.electronic_energy,
            self.0.nuclear_repulsion_energy,
            self.0.total_energy,
        ]
    }
    #[must_use]
    pub fn iterations(&self) -> u64 {
        self.0.iterations
    }
    /// Final absolute energy change (Hartree) and SCF residual norm.
    #[must_use]
    pub fn convergence(&self) -> [f64; 2] {
        [self.0.delta_energy, self.0.residual_norm]
    }
    /// S^2, ideal S^2, and spin contamination for UHF.
    #[must_use]
    pub fn spin(&self) -> Option<[f64; 3]> {
        self.0.spin
    }

    pub(super) fn validate(&self, data: &RustiQData) -> Result<(), ArtifactError> {
        let context = data
            .context
            .as_ref()
            .ok_or_else(|| invalid("HF requires resolved context"))?;
        let r = &self.0;
        if r.version != 1 {
            return Err(ArtifactError::UnsupportedRepresentation(format!(
                "HF version {}",
                r.version
            )));
        }
        if context
            .0
            .hf_identity()
            .map_err(|e| invalid(&e.to_string()))?
            != r.identity
        {
            return Err(invalid("HF identity disagrees with resolved context"));
        }
        if context.0.computation_version() != crate::persistence::AO_ERI_COMPUTATION_VERSION {
            return Err(ArtifactError::UnsupportedRepresentation(
                "HF AO computation version".into(),
            ));
        }
        let n = context.basis().len() as u64;
        let rank = r.effective_rank;
        if r.basis_dimension != n
            || rank == 0
            || rank > n
            || r.iterations == 0
            || r.iterations > context.max_iterations() as u64
        {
            return Err(invalid("invalid HF dimensions or iteration count"));
        }
        let finite = [
            r.electronic_energy,
            r.nuclear_repulsion_energy,
            r.total_energy,
            r.delta_energy,
            r.residual_norm,
            r.linear_dependency_threshold,
        ];
        if finite.iter().any(|v| !v.is_finite())
            || r.spin.is_some_and(|v| v.iter().any(|x| !x.is_finite()))
            || r.delta_energy < 0.0
            || r.residual_norm < 0.0
            || r.delta_energy >= context.convergence_threshold()
            || r.residual_norm >= context.convergence_threshold()
            || r.linear_dependency_threshold.to_bits()
                != context.linear_dependency_threshold().to_bits()
        {
            return Err(invalid("invalid HF convergence metadata"));
        }
        let electrons =
            context.atoms().map(|(z, _)| i64::from(z)).sum::<i64>() - i64::from(context.charge());
        let unpaired = i64::from(context.multiplicity()) - 1;
        let alpha = u64::try_from(electrons.midpoint(unpaired))
            .map_err(|_| invalid("invalid electrons"))?;
        let beta =
            u64::try_from((electrons - unpaired) / 2).map_err(|_| invalid("invalid electrons"))?;
        let check = |c: &HfOrbitalChannel, occupied| {
            if c.occupied != occupied
                || c.occupied > rank
                || c.energies.len() as u64 != rank
                || n.checked_mul(rank) != Some(c.coefficients.len() as u64)
                || c.coefficients
                    .iter()
                    .chain(&c.energies)
                    .any(|v| !v.is_finite())
            {
                Err(invalid(
                    "invalid HF orbital dimensions, occupations or values",
                ))
            } else {
                Ok(())
            }
        };
        match (&r.orbitals, context.hf_method()) {
            (HfOrbitals::Rhf { shared }, ResolvedHfMethod::Rhf) if r.spin.is_none() => {
                check(shared, alpha)
            }
            (HfOrbitals::Uhf { alpha: a, beta: b }, ResolvedHfMethod::Uhf) if r.spin.is_some() => {
                check(a, alpha)?;
                check(b, beta)
            }
            _ => Err(invalid("HF reference disagrees with context")),
        }
    }
}
fn invalid(message: &str) -> ArtifactError {
    ArtifactError::InvalidMetadata(message.into())
}

/// Typed marker for converged, inspection-only HF data.
pub struct ConvergedHfArtifact;
impl private::Sealed for ConvergedHfArtifact {}
impl Artifact for ConvergedHfArtifact {
    type Value = ConvergedHf;
    const NAME: &'static str = "converged_hf";
    fn is_present(data: &RustiQData) -> bool {
        data.hf.is_some() || data.manifest.artifacts.contains_key(Self::NAME)
    }
    fn is_compatible(data: &RustiQData, calculation: &PreparedCalculation) -> bool {
        let requested = crate::persistence::calculation::Snapshot::from_calculation(calculation)
            .ok()
            .and_then(|s| s.hf_identity().ok());
        requested.is_some()
            && requested == data.context.as_ref().and_then(|c| c.0.hf_identity().ok())
    }
    fn set(data: &mut RustiQData, value: ConvergedHf) -> Result<(), ArtifactError> {
        value.validate(data)?;
        data.hf = Some(value);
        Ok(())
    }
    fn get(data: &mut RustiQData) -> Result<Option<&ConvergedHf>, ArtifactError> {
        if data.hf.is_none() {
            let Some(a) = data.manifest.artifacts.get(Self::NAME) else {
                return Ok(None);
            };
            if !matches!(
                a.representation.as_str(),
                HF_REPRESENTATION | "rustiq-converged-hf-v1"
            ) || a.path.as_str() != HF_PATH
            {
                return Err(ArtifactError::UnsupportedRepresentation(
                    a.representation.clone(),
                ));
            }
            if a.size > MAX_HF_BYTES {
                return Err(invalid("HF payload exceeds size limit"));
            }
            let source = data.source.as_mut().ok_or(ArtifactError::Missing)?;
            if source.artifact_size(&a.path)? != a.size {
                return Err(ArtifactError::IntegrityMismatch(a.path.to_string()));
            }
            let metadata = source.artifact_metadata(&a.path)?;
            if metadata.size != a.size || metadata.digest != a.digest {
                return Err(ArtifactError::IntegrityMismatch(a.path.to_string()));
            }
            let record = if a.representation == "rustiq-converged-hf-v1" {
                source
                    .read_json(RelativePath::new(HF_PATH), MAX_HF_BYTES)
                    .map_err(|e| invalid(&e.to_string()))?
            } else {
                let record: HfRecord<HfOrbitals<StoredChannel>> = source
                    .read_json(RelativePath::new(HF_PATH), MAX_HF_BYTES)
                    .map_err(|e| invalid(&e.to_string()))?;
                record.restore(source)?
            };
            let value = ConvergedHf(Arc::new(record));
            value.validate(data)?;
            data.hf = Some(value);
        }
        Ok(data.hf.as_ref())
    }
}

impl RustiQData {
    /// Checks the supported HF context identity without loading orbital data.
    /// This does not verify payload integrity or make the artifact executable.
    #[must_use]
    pub fn hf_is_compatible(&self, calculation: &PreparedCalculation) -> bool {
        ConvergedHfArtifact::is_present(self)
            && (self.hf.is_some()
                || self
                    .manifest
                    .artifacts
                    .get(ConvergedHfArtifact::NAME)
                    .is_some_and(|a| {
                        matches!(
                            a.representation.as_str(),
                            HF_REPRESENTATION | "rustiq-converged-hf-v1"
                        )
                    }))
            && ConvergedHfArtifact::is_compatible(self, calculation)
    }

    pub(super) fn write_hf(
        &self,
        destination: &mut crate::persistence::Storage,
    ) -> Result<
        Option<crate::persistence::ArtifactManifest>,
        crate::persistence::PersistenceWriteError,
    > {
        let Some(hf) = &self.hf else {
            return Ok(None);
        };
        hf.validate(self)?;
        let record = hf.0.store(destination)?;
        let metadata = destination.write_json(RelativePath::new(HF_PATH), &record)?;
        if metadata.size > MAX_HF_BYTES {
            return Err(invalid("HF payload exceeds size limit").into());
        }
        Ok(Some(crate::persistence::ArtifactManifest {
            path: HF_PATH.into(),
            representation: HF_REPRESENTATION.into(),
            size: metadata.size,
            digest: metadata.digest,
            attributes: crate::persistence::ArtifactAttributes::Unknown(
                std::collections::BTreeMap::default(),
            ),
        }))
    }
}

#[cfg(test)]
mod tests;

macro_rules! persist_hf {
    ($reference:ty) => {
        impl crate::persistence::PersistScf for crate::calculation::HfMethodDescriptor<$reference> {
            type Artifact = ConvergedHfArtifact;
            fn to_artifact(
                solution: &crate::calculation::ScfSolution<Self>,
            ) -> Result<ConvergedHf, ArtifactError> {
                // Clone shares the solution allocation; conversion never copies retained ERIs.
                let runtime: AnyHfSolution = solution.clone().into();
                ConvergedHf::from_runtime(&runtime)
            }
        }
    };
}
persist_hf!(crate::calculation::Restricted);
persist_hf!(crate::calculation::Unrestricted);

use crate::calculation::Spin;
use crate::persistence::payload::{Payload, PayloadRef};
use crate::persistence::Storage;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredChannel {
    coefficients: PayloadRef,
    energies: PayloadRef,
    occupied: u64,
}

impl<O> HfRecord<O> {
    fn with_orbitals<P>(&self, orbitals: P) -> HfRecord<P> {
        HfRecord {
            version: self.version,
            identity: self.identity,
            basis_dimension: self.basis_dimension,
            effective_rank: self.effective_rank,
            iterations: self.iterations,
            electronic_energy: self.electronic_energy,
            nuclear_repulsion_energy: self.nuclear_repulsion_energy,
            total_energy: self.total_energy,
            delta_energy: self.delta_energy,
            residual_norm: self.residual_norm,
            linear_dependency_threshold: self.linear_dependency_threshold,
            spin: self.spin,
            orbitals,
        }
    }
}

impl HfRecord {
    fn store(
        &self,
        storage: &mut Storage,
    ) -> Result<HfRecord<HfOrbitals<StoredChannel>>, ArtifactError> {
        let channel = |storage: &mut Storage,
                       c: &HfOrbitalChannel,
                       stem: &str|
         -> Result<StoredChannel, ArtifactError> {
            let rows = usize::try_from(self.basis_dimension)
                .map_err(|_| invalid("HF dimension overflow"))?;
            let cols = c.energies.len();
            Ok(StoredChannel {
                coefficients: DMatrix::from_vec(rows, cols, c.coefficients.clone())
                    .save(storage, &format!("{stem}/coefficients"))?,
                energies: DVector::from_vec(c.energies.clone())
                    .save(storage, &format!("{stem}/energies"))?,
                occupied: c.occupied,
            })
        };
        let orbitals = match &self.orbitals {
            HfOrbitals::Rhf { shared } => HfOrbitals::Rhf {
                shared: channel(storage, shared, "orbitals")?,
            },
            HfOrbitals::Uhf { alpha, beta } => {
                let rows = usize::try_from(self.basis_dimension)
                    .map_err(|_| invalid("HF dimension overflow"))?;
                let channels = Spin::new(alpha, beta);
                let coefficients = channels
                    .map(|c| DMatrix::from_vec(rows, c.energies.len(), c.coefficients.clone()))
                    .save(storage, "orbitals/coefficients")?;
                let energies = channels
                    .map(|c| DVector::from_vec(c.energies.clone()))
                    .save(storage, "orbitals/energies")?;
                let stored = coefficients
                    .zip_map(energies, |coefficients, energies| (coefficients, energies))
                    .zip_map(channels, |(coefficients, energies), c| StoredChannel {
                        coefficients,
                        energies,
                        occupied: c.occupied,
                    });
                let (alpha, beta) = stored.into_parts();
                HfOrbitals::Uhf { alpha, beta }
            }
        };
        Ok(self.with_orbitals(orbitals))
    }
}

impl HfRecord<HfOrbitals<StoredChannel>> {
    fn restore(self, storage: &mut Storage) -> Result<HfRecord, ArtifactError> {
        let rows =
            usize::try_from(self.basis_dimension).map_err(|_| invalid("HF dimension overflow"))?;
        let cols =
            usize::try_from(self.effective_rank).map_err(|_| invalid("HF dimension overflow"))?;
        let channel = |storage: &mut Storage,
                       c: StoredChannel,
                       stem: &str|
         -> Result<HfOrbitalChannel, ArtifactError> {
            Ok(HfOrbitalChannel {
                coefficients: DMatrix::<f64>::load(
                    storage,
                    &format!("{stem}/coefficients"),
                    c.coefficients,
                    (rows, cols),
                )?
                .as_slice()
                .to_vec(),
                energies: DVector::<f64>::load(
                    storage,
                    &format!("{stem}/energies"),
                    c.energies,
                    cols,
                )?
                .as_slice()
                .to_vec(),
                occupied: c.occupied,
            })
        };
        // Move only the channel descriptors; the scalar metadata remains available.
        let record = self.with_orbitals(());
        let orbitals = match self.orbitals {
            HfOrbitals::Rhf { shared } => HfOrbitals::Rhf {
                shared: channel(storage, shared, "orbitals")?,
            },
            HfOrbitals::Uhf { alpha, beta } => {
                let occupied = Spin::new(alpha.occupied, beta.occupied);
                let coefficients = Spin::<DMatrix<f64>>::load(
                    storage,
                    "orbitals/coefficients",
                    Spin::new(alpha.coefficients, beta.coefficients),
                    (rows, cols),
                )?;
                let energies = Spin::<DVector<f64>>::load(
                    storage,
                    "orbitals/energies",
                    Spin::new(alpha.energies, beta.energies),
                    cols,
                )?;
                let channels = coefficients.zip_map(energies, |c, e| (c, e)).zip_map(
                    occupied,
                    |(c, e), occupied| HfOrbitalChannel {
                        coefficients: c.as_slice().to_vec(),
                        energies: e.as_slice().to_vec(),
                        occupied,
                    },
                );
                let (alpha, beta) = channels.into_parts();
                HfOrbitals::Uhf { alpha, beta }
            }
        };
        Ok(record.with_orbitals(orbitals))
    }
}
