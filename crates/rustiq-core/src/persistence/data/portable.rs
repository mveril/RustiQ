use super::super::{
    calculation::{Snapshot, CALCULATION_PATH, MAX_CALCULATION_BYTES},
    provenance::{MAX_SOURCES, MAX_SOURCE_BYTES, MAX_SOURCE_NAME_BYTES},
    request::{RequestSnapshot, MAX_REQUEST_BYTES, REQUEST_PATH},
    ArtifactAttributes, ArtifactError, CalculationContext, ManifestKind, PortableError,
    SourceProvenance, Storage, AO_ERI_PATH, COMPACT_ERI_REPRESENTATION,
};
use super::{RustiQData, AO_ERI_ARTIFACT};
use crate::calculation::PreparedCalculation;
use relative_path::RelativePath;
use std::io::Read;
use std::path::Path;

impl RustiQData {
    /// Creates portable scientific data from effective inputs, without running HF.
    /// Add known artifacts with the typed setters before calling [`Self::write`].
    ///
    /// # Errors
    ///
    /// Returns an error if resolved inputs or their portable request are invalid or inconsistent.
    pub fn from_calculation(calculation: &PreparedCalculation) -> Result<Self, PortableError> {
        let snapshot = Snapshot::from_calculation(calculation)?;
        RequestSnapshot::from_request(calculation.request())?;
        snapshot.validate_request(calculation.request())?;
        let mut data = Self::new_with_identity(
            snapshot.identity(),
            snapshot.basis_functions(),
            ManifestKind::Portable,
        );
        data.request = Some(calculation.request().clone());
        data.context = Some(CalculationContext::from_snapshot(snapshot)?);
        Ok(data)
    }

    /// Opens a portable archive and validates its context and artifact index.
    /// Numerical payloads are verified and decoded lazily on first access.
    /// Multi-calculation archives require [`super::RustiQBundle::open`].
    ///
    /// # Errors
    ///
    /// Returns an error for inaccessible, unsupported, corrupt, or multi-calculation archives.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PortableError> {
        let bundle = super::RustiQBundle::open(path)?;
        let (mut calculations, sources) = bundle.into_parts();
        if calculations.len() != 1 {
            return Err(PortableError::InvalidArchive(
                "single-calculation API requires exactly one calculation".into(),
            ));
        }
        let mut data = calculations.remove(0);
        data.sources = sources;
        Ok(data)
    }

    pub(crate) fn validate_portable(mut data: Self) -> Result<Self, PortableError> {
        if data.manifest.kind != ManifestKind::Portable {
            return Err(PortableError::UnsupportedVersion);
        }
        let source = data.source.as_mut().expect("opened storage");
        let reference =
            data.manifest.calculation.as_ref().ok_or_else(|| {
                PortableError::InvalidArchive("missing calculation reference".into())
            })?;
        verify_reference(source, reference, CALCULATION_PATH, MAX_CALCULATION_BYTES)?;
        let snapshot: Snapshot = source
            .read_json(RelativePath::new(CALCULATION_PATH), MAX_CALCULATION_BYTES)
            .map_err(|error| PortableError::InvalidCalculation(error.to_string()))?;
        snapshot.validate()?;
        let reference = data
            .manifest
            .request
            .as_ref()
            .ok_or_else(|| PortableError::InvalidArchive("missing request reference".into()))?;
        verify_reference(source, reference, REQUEST_PATH, MAX_REQUEST_BYTES)?;
        let request_snapshot: RequestSnapshot = source
            .read_json(RelativePath::new(REQUEST_PATH), MAX_REQUEST_BYTES)
            .map_err(|error| PortableError::InvalidRequest(error.to_string()))?;
        let request = request_snapshot.to_request()?;
        snapshot.validate_request(&request)?;
        data.request = Some(request);
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
                        || part.eq_ignore_ascii_case(REQUEST_PATH)
                        || part.eq_ignore_ascii_case("sources")
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
        data.context = Some(CalculationContext::from_snapshot(snapshot)?);
        Ok(data)
    }

    /// Returns the normalized pre-resolution request, absent for directory-cache data.
    #[must_use]
    pub fn request(&self) -> Option<&crate::calculation::CalculationRequest> {
        self.request.as_ref()
    }

    /// Rebuilds an executable calculation from the resolved context in this portable archive.
    /// The archive becomes a read-only source of typed artifacts for normal execution.
    /// Payloads are loaded and verified only when their scientific stage needs them.
    /// Missing or incompatible artifacts fall back to computation; corrupt compatible
    /// artifacts produce an execution error. The source archive is never modified.
    ///
    /// # Errors
    ///
    /// Returns an error if portable context or request data is missing or incompatible.
    pub fn prepare_calculation(self) -> Result<PreparedCalculation, PortableError> {
        let context = self.context.as_ref().ok_or_else(|| {
            PortableError::InvalidCalculation(
                "calculation preparation requires portable calculation context".into(),
            )
        })?;
        let request = self.request.as_ref().ok_or_else(|| {
            PortableError::InvalidCalculation(
                "calculation preparation requires a portable request".into(),
            )
        })?;
        let prepared = PreparedCalculation::from_portable_context(request.clone(), context);
        prepared.with_reuse_data(self)
    }

    /// Captured sources are opaque provenance and never affect scientific compatibility.
    #[must_use = "Consume the iterator to inspect the stored scientific data"]
    pub fn sources(&self) -> impl ExactSizeIterator<Item = &SourceProvenance> {
        self.sources.iter()
    }

    /// Captures exact source bytes; the original name is informational, never an archive path.
    ///
    /// # Errors
    ///
    /// Returns an error if portable context is absent or provenance exceeds supported size or count limits.
    pub fn add_source(
        &mut self,
        original_name: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<(), PortableError> {
        if self.context.is_none() {
            return Err(PortableError::InvalidCalculation(
                "source provenance requires portable context".into(),
            ));
        }
        let original_name = original_name.into();
        let bytes = bytes.into();
        let total = self
            .sources
            .iter()
            .map(|source| source.bytes().len())
            .chain(std::iter::once(bytes.len()))
            .try_fold(0_u64, |total, length| {
                u64::try_from(length)
                    .ok()
                    .and_then(|size| total.checked_add(size))
            })
            .ok_or_else(|| PortableError::InvalidArchive("source size exceeds V1 range".into()))?;
        if self.sources.len() >= MAX_SOURCES
            || total > MAX_SOURCE_BYTES
            || original_name.len() > MAX_SOURCE_NAME_BYTES
        {
            return Err(PortableError::InvalidArchive(
                "source provenance exceeds supported limits".into(),
            ));
        }
        self.sources.push(SourceProvenance {
            original_name,
            bytes,
        });
        Ok(())
    }

    /// Returns the portable context, absent only for internal directory-cache data.
    #[must_use]
    pub fn calculation(&self) -> Option<&CalculationContext> {
        self.context.as_ref()
    }

    /// Producer name and version recorded as provenance, not compatibility policy.
    #[must_use]
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
    #[must_use]
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

    pub(super) fn matches_eri_identity(&self, calculation: &PreparedCalculation) -> bool {
        let identity = super::super::ao_eri_identity(
            calculation.get_molecule(),
            calculation.get_basis(),
            calculation.integral_config().schwarz_threshold.value,
        );
        self.manifest.scientific_identity.version == identity.version
            && self.manifest.scientific_identity.digest == identity.digest
    }

    /// Atomically publishes a portable archive. Never overwrites an existing path.
    /// The source archive is not modified; unloaded artifacts are copied and verified.
    ///
    /// # Errors
    ///
    /// Returns an error if portable context is missing, artifact validation fails, or atomic publication fails.
    pub fn write(&mut self, path: impl AsRef<Path>) -> Result<(), PortableError> {
        if self.context.is_none()
            || self.request.is_none()
            || self.manifest.kind != ManifestKind::Portable
        {
            return Err(PortableError::InvalidCalculation(
                "portable output requires resolved calculation context".into(),
            ));
        }
        let sources = std::mem::take(&mut self.sources);
        let result =
            super::bundle::write_bundle(std::slice::from_mut(self), &sources, path.as_ref());
        self.sources = sources;
        result
    }

    /// Writes a single-calculation V1 bundle with a borrowed AO ERI tensor.
    ///
    /// # Errors
    ///
    /// Returns an error if the supplied ERI is incompatible or archive validation or publication fails.
    pub fn write_with_eri(
        &mut self,
        path: impl AsRef<Path>,
        eri: &crate::eri::CompactEri,
    ) -> Result<(), PortableError> {
        self.validate_eri(eri)?;
        let sources = std::mem::take(&mut self.sources);
        let result = super::bundle::write_bundle_with_eri(
            std::slice::from_mut(self),
            &sources,
            path.as_ref(),
            Some(eri),
        );
        self.sources = sources;
        result
    }

    /// Validates compatibility and lazily decodes the ERI, then transfers ownership.
    /// Missing, unsupported, incompatible, or corrupt artifacts are errors.
    ///
    /// # Errors
    ///
    /// Returns an error if the ERI is absent, incompatible, corrupt, or cannot be decoded.
    ///
    /// # Panics
    ///
    /// Panics if a validated, loaded ERI is unexpectedly absent from internal state.
    pub fn take_compatible_eri(
        &mut self,
        calculation: &PreparedCalculation,
    ) -> Result<crate::eri::CompactEri, PortableError> {
        if self.ao_eri.is_none() {
            let artifact = self
                .manifest
                .artifacts
                .get(AO_ERI_ARTIFACT)
                .ok_or(ArtifactError::Missing)?;
            if artifact.representation != COMPACT_ERI_REPRESENTATION {
                return Err(ArtifactError::UnsupportedRepresentation(
                    artifact.representation.clone(),
                )
                .into());
            }
        }
        if !self.eri_is_compatible(calculation) {
            return Err(PortableError::IncompatibleEri);
        }
        self.read_eri()?;
        Ok(self.take_eri().expect("validated ERI is loaded"))
    }

    pub(crate) fn validate_unloaded_eri(&mut self) -> Result<(), PortableError> {
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

pub(super) fn read_sources(
    source: &mut Storage,
    references: &[super::super::manifest::SourceManifest],
) -> Result<Vec<SourceProvenance>, PortableError> {
    let mut sources = Vec::new();
    if references.len() > MAX_SOURCES {
        return Err(PortableError::InvalidArchive(
            "too many source payloads".into(),
        ));
    }
    let mut total = 0_u64;
    let mut paths = std::collections::HashSet::new();
    for reference in references {
        let path = RelativePath::new(&reference.path);
        super::super::validate_storage_path(path)?;
        total = total
            .checked_add(reference.size)
            .filter(|total| *total <= MAX_SOURCE_BYTES)
            .ok_or_else(|| {
                PortableError::InvalidArchive("source payloads exceed size limit".into())
            })?;
        if reference.version != 1 {
            return Err(PortableError::UnsupportedVersion);
        }
        if !reference.path.starts_with("sources/")
            || reference.original_name.len() > MAX_SOURCE_NAME_BYTES
            || !paths.insert(reference.path.to_lowercase())
        {
            return Err(PortableError::InvalidArchive(
                "invalid source reference".into(),
            ));
        }
        let expected = super::super::manifest::SnapshotManifest {
            path: reference.path.clone(),
            version: reference.version,
            size: reference.size,
            digest: reference.digest,
        };
        verify_reference(source, &expected, &reference.path, MAX_SOURCE_BYTES)?;
        let bytes = source.with_artifact::<_, PortableError, _>(path, |reader| {
            let mut bytes = Vec::new();
            reader.take(reference.size + 1).read_to_end(&mut bytes)?;
            if u64::try_from(bytes.len())
                .map_err(|_| PortableError::InvalidArchive("source size exceeds V1 range".into()))?
                != reference.size
            {
                return Err(ArtifactError::IntegrityMismatch(reference.path.clone()).into());
            }
            Ok(bytes)
        })?;
        sources.push(SourceProvenance {
            original_name: reference.original_name.clone(),
            bytes,
        });
    }
    Ok(sources)
}

fn verify_reference(
    source: &mut Storage,
    reference: &super::super::manifest::SnapshotManifest,
    expected_path: &str,
    limit: u64,
) -> Result<(), PortableError> {
    if reference.version != 1 {
        return Err(PortableError::UnsupportedVersion);
    }
    if reference.path != expected_path || reference.size > limit {
        return Err(PortableError::InvalidArchive(
            "invalid snapshot/source reference".into(),
        ));
    }
    let path = RelativePath::new(expected_path);
    if source.artifact_size(path)? != reference.size {
        return Err(ArtifactError::IntegrityMismatch(expected_path.into()).into());
    }
    let metadata = source.artifact_metadata(path)?;
    if metadata.size != reference.size || metadata.digest != reference.digest {
        return Err(ArtifactError::IntegrityMismatch(expected_path.into()).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
