use super::super::{
    calculation::{Snapshot, CALCULATION_PATH, MAX_CALCULATION_BYTES},
    ArtifactAttributes, ArtifactError, CalculationContext, ManifestKind, PortableError, Storage,
    AO_ERI_PATH, COMPACT_ERI_REPRESENTATION,
};
use super::{RustiQData, AO_ERI_ARTIFACT};
use crate::calculation::PreparedCalculation;
use relative_path::RelativePath;
#[cfg(any(unix, test))]
use std::fs::File;
use std::path::Path;

impl RustiQData {
    /// Creates portable scientific data from effective inputs, without running HF.
    /// Add known artifacts with the typed setters before calling [`Self::write`].
    pub fn from_calculation(calculation: &PreparedCalculation) -> Result<Self, PortableError> {
        let snapshot = Snapshot::from_calculation(calculation)?;
        let mut data = Self::new_with_identity(
            snapshot.identity(),
            snapshot.basis_functions(),
            ManifestKind::Portable,
        );
        data.context = Some(CalculationContext(snapshot));
        Ok(data)
    }

    /// Opens a portable archive and validates its context and artifact index.
    /// Numerical payloads are verified and decoded lazily on first access.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PortableError> {
        let mut data = Self::read_from(Storage::open_zip(path.as_ref())?)?;
        if data.manifest.kind != ManifestKind::Portable {
            return Err(PortableError::UnsupportedVersion);
        }
        let reference =
            data.manifest.calculation.as_ref().ok_or_else(|| {
                PortableError::InvalidArchive("missing calculation reference".into())
            })?;
        if reference.version != 1 {
            return Err(PortableError::UnsupportedVersion);
        }
        if reference.path != CALCULATION_PATH || reference.size > MAX_CALCULATION_BYTES {
            return Err(PortableError::InvalidArchive(
                "invalid calculation reference".into(),
            ));
        }
        let source = data.source.as_mut().expect("opened storage");
        let path = RelativePath::new(CALCULATION_PATH);
        if source.artifact_size(path)? != reference.size {
            return Err(ArtifactError::IntegrityMismatch(CALCULATION_PATH.into()).into());
        }
        let metadata = source.artifact_metadata(path)?;
        if metadata.digest != reference.digest {
            return Err(ArtifactError::IntegrityMismatch(CALCULATION_PATH.into()).into());
        }
        let snapshot: Snapshot = source
            .read_json(path, MAX_CALCULATION_BYTES)
            .map_err(|error| PortableError::InvalidCalculation(error.to_string()))?;
        snapshot.validate()?;
        let identity = snapshot.identity();
        if identity.version != data.manifest.scientific_identity.version
            || identity.digest != data.manifest.scientific_identity.digest
        {
            return Err(PortableError::InvalidCalculation(
                "scientific identity does not match the resolved inputs".into(),
            ));
        }
        for (name, artifact) in &data.manifest.artifacts {
            if artifact
                .path
                .as_str()
                .split('/')
                .next()
                .is_some_and(|part| {
                    part.eq_ignore_ascii_case(CALCULATION_PATH)
                        || part.eq_ignore_ascii_case("manifest.json")
                })
            {
                return Err(ArtifactError::InvalidPath(artifact.path.to_string()).into());
            }
            if source.artifact_size(&artifact.path)? != artifact.size {
                return Err(ArtifactError::IntegrityMismatch(artifact.path.to_string()).into());
            }
            if let ArtifactAttributes::AoEri(attributes) = &artifact.attributes {
                if name != AO_ERI_ARTIFACT
                    || artifact.path.as_str() != AO_ERI_PATH
                    || attributes.basis_functions != snapshot.basis_functions()
                    || attributes.computation_version != snapshot.computation_version()
                {
                    return Err(ArtifactError::InvalidMetadata(
                        "AO ERI metadata disagrees with calculation context".into(),
                    )
                    .into());
                }
            }
        }
        data.basis_functions = Some(snapshot.basis_functions());
        data.context = Some(CalculationContext(snapshot));
        Ok(data)
    }

    /// Returns the portable context, absent only for internal directory-cache data.
    pub fn calculation(&self) -> Option<&CalculationContext> {
        self.context.as_ref()
    }

    /// Producer name and version recorded as provenance, not compatibility policy.
    pub fn producer(&self) -> (&str, &str) {
        (
            &self.manifest.producer.name,
            &self.manifest.producer.version,
        )
    }

    /// Lists scientific artifact names and representation identifiers without loading arrays.
    /// Unknown representations remain visible; listing does not verify their payloads.
    pub fn artifact_representations(&self) -> impl Iterator<Item = (&str, &str)> {
        self.ao_eri
            .as_ref()
            .map(|_| (AO_ERI_ARTIFACT, COMPACT_ERI_REPRESENTATION))
            .into_iter()
            .chain(
                self.manifest
                    .artifacts
                    .iter()
                    .filter_map(|(name, artifact)| {
                        (name != AO_ERI_ARTIFACT || self.ao_eri.is_none())
                            .then_some((name.as_str(), artifact.representation.as_str()))
                    }),
            )
    }

    /// Tests AO ERI compatibility with another resolved calculation, without reading arrays.
    /// Requested HF/MP2 methods do not participate in deterministic integral identity.
    pub fn eri_is_compatible(&self, calculation: &PreparedCalculation) -> bool {
        if self.ao_eri.is_some() {
            return self.matches_eri_identity(calculation);
        }
        let Some(artifact) = self.manifest.artifacts.get(AO_ERI_ARTIFACT) else {
            return false;
        };
        matches!(&artifact.attributes, ArtifactAttributes::AoEri(attributes)
            if attributes.computation_version == super::super::AO_ERI_COMPUTATION_VERSION)
            && artifact.representation == COMPACT_ERI_REPRESENTATION
            && self.matches_eri_identity(calculation)
    }

    fn matches_eri_identity(&self, calculation: &PreparedCalculation) -> bool {
        let hf = calculation.hf_config();
        let identity = super::super::ao_eri_identity(
            calculation.get_molecule(),
            calculation.get_basis(),
            hf.eri_schwarz_threshold,
        );
        self.manifest.scientific_identity.version == identity.version
            && self.manifest.scientific_identity.digest == identity.digest
    }

    /// Atomically publishes a portable archive. Never overwrites an existing path.
    /// The source archive is not modified; unloaded artifacts are copied and verified.
    pub fn write(&mut self, path: impl AsRef<Path>) -> Result<(), PortableError> {
        if self.context.is_none() || self.manifest.kind != ManifestKind::Portable {
            return Err(PortableError::InvalidCalculation(
                "portable output requires resolved calculation context".into(),
            ));
        }
        // Validate any known, unloaded artifact before preserving it as scientific data.
        if self.ao_eri.is_none() && self.manifest.artifacts.get(AO_ERI_ARTIFACT)
            .is_some_and(|artifact| artifact.representation == COMPACT_ERI_REPRESENTATION
                && matches!(&artifact.attributes, ArtifactAttributes::AoEri(attributes) if attributes.computation_version == super::super::AO_ERI_COMPUTATION_VERSION)) {
            self.validate_unloaded_eri()?;
        }
        let path = path.as_ref();
        match std::fs::symlink_metadata(path) {
            Ok(_) => return Err(PortableError::AlreadyExists),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let temporary = tempfile::NamedTempFile::new_in(parent)?;
        self.write_to(Storage::create_zip(temporary.reopen()?))?;
        // Reopening validates the finalized container, context, index and publication limits.
        drop(Self::open(temporary.path())?);
        temporary.persist_noclobber(path).map_err(|error| {
            if error.error.kind() == std::io::ErrorKind::AlreadyExists {
                PortableError::AlreadyExists
            } else {
                PortableError::Io(error.error)
            }
        })?;
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        Ok(())
    }

    fn validate_unloaded_eri(&mut self) -> Result<(), PortableError> {
        let artifact = &self.manifest.artifacts[AO_ERI_ARTIFACT];
        let ArtifactAttributes::AoEri(attributes) = &artifact.attributes else {
            unreachable!()
        };
        let source = self.source.as_mut().ok_or(ArtifactError::Missing)?;
        source.with_artifact::<_, ArtifactError, _>(&artifact.path, |reader| {
            super::super::npy::checked_eri_prefix(
                reader,
                attributes.basis_functions,
                artifact.size,
            )?;
            Ok(())
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
