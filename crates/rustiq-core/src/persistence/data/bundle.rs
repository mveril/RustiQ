//! A non-empty portable collection; scientific APIs still operate on one calculation.
use std::{
    collections::HashSet,
    path::Path,
    sync::{Arc, Mutex},
};

use relative_path::{RelativePath, RelativePathBuf};

use super::{portable, rustiq_data::MAX_MANIFEST_BYTES, RustiQData};
use crate::persistence::{
    manifest::{PortableCalculation, PortableManifest, SourceManifest},
    ArtifactAttributes, Manifest, ManifestKind, PortableError, Producer, SourceProvenance, Storage,
    COMPACT_ERI_REPRESENTATION, FORMAT_NAME, FORMAT_VERSION, MANIFEST_PATH,
};

/// A portable snapshot containing one or more independently reusable calculations.
/// Sources are shared opaque provenance. ZIP details remain private.
#[derive(Debug)]
pub struct RustiQBundle {
    calculations: Vec<RustiQData>,
    sources: Vec<SourceProvenance>,
}

impl RustiQBundle {
    /// Collects portable calculation data, rejecting empty or cache-only collections.
    /// Exact duplicate sources are stored once at the bundle level.
    ///
    /// # Errors
    ///
    /// Returns an error if the collection is empty or any calculation lacks portable context or request data.
    pub fn new(mut calculations: Vec<RustiQData>) -> Result<Self, PortableError> {
        if calculations.is_empty() {
            return Err(invalid("portable bundles require at least one calculation"));
        }
        let mut sources = Vec::new();
        for data in &mut calculations {
            if data.request.is_none() || data.context.is_none() {
                return Err(invalid(
                    "portable entries require request and resolved snapshots",
                ));
            }
            for source in &data.sources {
                if !sources.contains(source) {
                    sources.push(source.clone());
                }
            }
        }
        validate_sources(&sources)?;
        for data in &mut calculations {
            data.sources.clear();
        }
        Ok(Self {
            calculations,
            sources,
        })
    }

    /// Opens read-only, validating all snapshots and references without loading arrays.
    ///
    /// # Errors
    ///
    /// Returns an error if archive structure, context, references, or provenance are invalid or cannot be read.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PortableError> {
        let mut storage = Storage::open_zip(path.as_ref())?;
        let manifest: PortableManifest = storage
            .read_json(RelativePath::new(MANIFEST_PATH), MAX_MANIFEST_BYTES)
            .map_err(crate::persistence::PersistenceReadError::from)?;
        if manifest.format != FORMAT_NAME
            || manifest.format_version != FORMAT_VERSION
            || manifest.kind != ManifestKind::Portable
        {
            return Err(PortableError::UnsupportedVersion);
        }
        if manifest.calculations.is_empty() {
            return Err(invalid("portable bundles require at least one calculation"));
        }
        let sources = portable::read_sources(&mut storage, &manifest.sources)?;
        let storage = Arc::new(Mutex::new(storage));
        let mut ids = HashSet::new();
        let mut calculations = Vec::new();
        for entry in manifest.calculations {
            let digits = entry.id.strip_prefix("calculation-").unwrap_or_default();
            if digits.is_empty()
                || !digits.bytes().all(|b| b.is_ascii_digit())
                || !ids.insert(entry.id.clone())
            {
                return Err(invalid("invalid or duplicate calculation identifier"));
            }
            let prefix = format!("calculations/{}", entry.id);
            let mut request = entry.request;
            let mut calculation = entry.calculation;
            request.path = local_path(&prefix, &request.path)?;
            calculation.path = local_path(&prefix, &calculation.path)?;
            let mut artifacts = entry.artifacts;
            for artifact in artifacts.values_mut() {
                artifact.path = RelativePathBuf::from(local_path(&prefix, artifact.path.as_str())?);
            }
            let internal = Manifest {
                format: manifest.format.clone(),
                format_version: manifest.format_version,
                kind: ManifestKind::Portable,
                producer: manifest.producer.clone(),
                scientific_identity: entry.scientific_identity,
                artifacts,
                request: Some(request),
                calculation: Some(calculation),
                sources: Vec::new(),
            };
            let data =
                RustiQData::read_manifest(Storage::scoped(storage.clone(), prefix), internal)?;
            calculations.push(RustiQData::validate_portable(data)?);
        }
        Ok(Self {
            calculations,
            sources,
        })
    }

    /// Entries retain manifest order; each exposes its own typed artifact access.
    #[must_use]
    pub fn calculations(&self) -> &[RustiQData] {
        &self.calculations
    }

    /// Mutates scientific artifacts within entries, without allowing an empty collection.
    pub fn calculations_mut(&mut self) -> &mut [RustiQData] {
        &mut self.calculations
    }

    #[must_use = "Consume the iterator to inspect the stored scientific data"]
    pub fn sources(&self) -> impl ExactSizeIterator<Item = &SourceProvenance> {
        self.sources.iter()
    }

    /// Adds shared exact source bytes; the original name never becomes a member path.
    ///
    /// # Errors
    ///
    /// Returns an error if provenance exceeds supported size, name, or count limits.
    pub fn add_source(
        &mut self,
        original_name: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<(), PortableError> {
        let source = SourceProvenance {
            original_name: original_name.into(),
            bytes: bytes.into(),
        };
        let mut sources = self.sources.clone();
        sources.push(source);
        validate_sources(&sources)?;
        self.sources = sources;
        Ok(())
    }

    /// Publishes a complete validated snapshot atomically, without overwriting a destination.
    ///
    /// # Errors
    ///
    /// Returns an error if provenance or scientific artifacts are invalid or atomic publication fails.
    pub fn write(&mut self, path: impl AsRef<Path>) -> Result<(), PortableError> {
        // Sources attached through individual entry APIs are lifted to shared provenance.
        for data in &mut self.calculations {
            for source in &data.sources {
                if !self.sources.contains(source) {
                    self.sources.push(source.clone());
                }
            }
        }
        validate_sources(&self.sources)?;
        write_bundle(&mut self.calculations, &self.sources, path.as_ref())
    }

    pub(super) fn into_parts(self) -> (Vec<RustiQData>, Vec<SourceProvenance>) {
        (self.calculations, self.sources)
    }
}

fn invalid(message: &str) -> PortableError {
    PortableError::InvalidArchive(message.into())
}

fn local_path(prefix: &str, path: &str) -> Result<String, PortableError> {
    crate::persistence::validate_storage_path(RelativePath::new(path))?;
    path.strip_prefix(&format!("{prefix}/"))
        .map(str::to_owned)
        .ok_or_else(|| invalid("reference crosses calculation namespace"))
}

fn validate_sources(sources: &[SourceProvenance]) -> Result<(), PortableError> {
    use crate::persistence::provenance::{MAX_SOURCES, MAX_SOURCE_BYTES, MAX_SOURCE_NAME_BYTES};
    let total = sources.iter().try_fold(0_u64, |total, source| {
        total.checked_add(u64::try_from(source.bytes.len()).ok()?)
    });
    if sources.len() > MAX_SOURCES
        || total.is_none_or(|total| total > MAX_SOURCE_BYTES)
        || sources
            .iter()
            .any(|source| source.original_name.len() > MAX_SOURCE_NAME_BYTES)
    {
        return Err(invalid("source provenance exceeds supported limits"));
    }
    Ok(())
}

pub(super) fn write_bundle(
    calculations: &mut [RustiQData],
    sources: &[SourceProvenance],
    path: &Path,
) -> Result<(), PortableError> {
    write_bundle_with_eri(calculations, sources, path, None)
}

pub(super) fn write_bundle_with_eri(
    calculations: &mut [RustiQData],
    sources: &[SourceProvenance],
    path: &Path,
    eri: Option<&crate::eri::CompactEri>,
) -> Result<(), PortableError> {
    if calculations.is_empty() {
        return Err(invalid("portable bundles require at least one calculation"));
    }
    if eri.is_some() && calculations.len() != 1 {
        return Err(invalid(
            "a borrowed AO ERI can only be written with a single calculation",
        ));
    }
    validate_sources(sources)?;
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
    write_snapshot_inner(
        calculations,
        sources,
        Storage::create_zip(temporary.reopen()?),
        eri,
    )?;
    drop(RustiQBundle::open(temporary.path())?);
    temporary.persist_noclobber(path).map_err(|error| {
        if error.error.kind() == std::io::ErrorKind::AlreadyExists {
            PortableError::AlreadyExists
        } else {
            PortableError::Io(error.error)
        }
    })?;
    #[cfg(unix)]
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}

pub(super) fn write_snapshot(
    calculations: &mut [RustiQData],
    sources: &[SourceProvenance],
    destination: Storage,
) -> Result<(), PortableError> {
    write_snapshot_inner(calculations, sources, destination, None)
}

fn write_snapshot_inner(
    calculations: &mut [RustiQData],
    sources: &[SourceProvenance],
    destination: Storage,
    eri: Option<&crate::eri::CompactEri>,
) -> Result<(), PortableError> {
    let storage = Arc::new(Mutex::new(destination));
    let mut manifest = PortableManifest {
        format: FORMAT_NAME.into(),
        format_version: FORMAT_VERSION,
        kind: ManifestKind::Portable,
        producer: Producer {
            name: "RustiQ".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        sources: Vec::new(),
        calculations: Vec::new(),
    };
    for (index, source) in sources.iter().enumerate() {
        let path = format!("sources/{index}");
        let metadata = storage
            .lock()
            .expect("storage mutex poisoned")
            .write_artifact::<PortableError, _>(RelativePath::new(&path), |writer| {
                writer.write_all(source.bytes())?;
                Ok(())
            })?;
        manifest.sources.push(SourceManifest {
            original_name: source.original_name.clone(),
            path,
            version: 1,
            size: metadata.size,
            digest: metadata.digest,
        });
    }
    for (index, data) in calculations.iter_mut().enumerate() {
        if data.context.is_none() || data.request.is_none() {
            return Err(invalid("missing calculation snapshots"));
        }
        if eri.is_none() && data.ao_eri.is_none() && data.manifest.artifacts.get(super::AO_ERI_ARTIFACT).is_some_and(|artifact|
            artifact.representation == COMPACT_ERI_REPRESENTATION && matches!(&artifact.attributes,
                ArtifactAttributes::AoEri(attributes) if attributes.computation_version == crate::persistence::AO_ERI_COMPUTATION_VERSION)) {
            data.validate_unloaded_eri()?;
        }
        let id = format!("calculation-{index}");
        let prefix = format!("calculations/{id}");
        let saved_sources = std::mem::take(&mut data.sources);
        let destination = Storage::scoped(storage.clone(), prefix.clone());
        let result = match eri {
            Some(eri) => data.write_entry_with_eri(destination, eri),
            None => data.write_entry(destination),
        };
        data.sources = saved_sources;
        let entry = result?;
        let mut request = entry
            .request
            .ok_or_else(|| invalid("missing request snapshot"))?;
        let mut calculation = entry
            .calculation
            .ok_or_else(|| invalid("missing resolved snapshot"))?;
        request.path = format!("{prefix}/{}", request.path);
        calculation.path = format!("{prefix}/{}", calculation.path);
        let mut artifacts = entry.artifacts;
        for artifact in artifacts.values_mut() {
            artifact.path = RelativePathBuf::from(format!("{prefix}/{}", artifact.path));
        }
        manifest.calculations.push(PortableCalculation {
            id,
            scientific_identity: entry.scientific_identity,
            request,
            calculation,
            artifacts,
        });
    }
    storage
        .lock()
        .expect("storage mutex poisoned")
        .write_json(RelativePath::new(MANIFEST_PATH), &manifest)
        .map_err(crate::persistence::PersistenceWriteError::from)?;
    Arc::try_unwrap(storage)
        .expect("all entry writers released")
        .into_inner()
        .expect("storage mutex poisoned")
        .finish()?;
    Ok(())
}
