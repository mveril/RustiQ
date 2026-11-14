use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use relative_path::RelativePath;

use crate::eri::CompactEri;

use super::super::{
    validate_storage_path, write_compact_eri, AoEriAttributes, ArtifactAttributes, ArtifactError,
    ArtifactManifest, Manifest, ManifestError, ManifestKind, PersistenceReadError,
    PersistenceWriteError, Producer, ScientificIdentityManifest, Storage, StorageError,
    AO_ERI_COMPUTATION_VERSION, AO_ERI_PATH, COMPACT_ERI_REPRESENTATION, FORMAT_NAME,
    FORMAT_VERSION, MANIFEST_PATH,
};
use super::artifact::Artifact;

pub(crate) const AO_ERI_ARTIFACT: &str = "ao_eri";
pub(super) const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

/// Known scientific artifacts and their manifest, with values loaded on demand.
/// Cloning copies metadata and provenance while sharing disk-backed payloads.
/// Replacing a payload affects only that data instance.
#[derive(Clone, Debug)]
pub struct RustiQData {
    pub(super) manifest: Manifest,
    pub(super) source: Option<Arc<Storage>>,
    // TODO: Separate reader and writer responsibilities into two structures.
    pub(super) staged: BTreeMap<String, Arc<(tempfile::TempDir, Storage)>>,
    pub(super) basis_functions: Option<usize>,
    pub(super) request: Option<crate::calculation::CalculationRequest>,
    pub(super) sources: Vec<super::super::SourceProvenance>,
    pub(super) context: Option<super::super::CalculationContext>,
}

impl RustiQData {
    /// Gets a known scientific artifact, reading and validating it on every access.
    ///
    /// # Errors
    ///
    /// Returns an error if the artifact metadata, stored payload, or requested representation is
    /// invalid, or reading fails.
    pub fn get<A: Artifact>(&self) -> Result<Option<A::Value>, ArtifactError> {
        A::get(self)
    }

    /// Borrows and immediately stages a known scientific artifact on disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the artifact cannot be represented by the current metadata or
    /// writing its temporary on-disk payload fails.
    pub fn set<A: Artifact>(&mut self, value: &A::Value) -> Result<(), ArtifactError> {
        A::set(self, value)
    }

    pub(crate) fn new_with_identity(
        identity: super::super::ScientificIdentity,
        basis_functions: usize,
        kind: ManifestKind,
    ) -> Self {
        Self {
            manifest: Manifest {
                format: FORMAT_NAME.to_owned(),
                format_version: FORMAT_VERSION,
                kind,
                producer: Producer {
                    name: "RustiQ".to_owned(),
                    version: env!("CARGO_PKG_VERSION").to_owned(),
                },
                scientific_identity: ScientificIdentityManifest {
                    version: identity.version,
                    digest: identity.digest,
                },
                artifacts: BTreeMap::new(),
                calculation: None,
                request: None,
                sources: Vec::new(),
            },
            source: None,
            context: None,
            request: None,
            sources: Vec::new(),
            staged: BTreeMap::new(),
            basis_functions: Some(basis_functions),
        }
    }

    pub(crate) fn read_from(source: Storage) -> Result<Self, PersistenceReadError> {
        let manifest: Manifest =
            source.read_json(RelativePath::new(MANIFEST_PATH), MAX_MANIFEST_BYTES)?;
        Self::read_manifest(source, manifest)
    }

    pub(crate) fn read_manifest(
        source: Storage,
        manifest: Manifest,
    ) -> Result<Self, PersistenceReadError> {
        if manifest.format != FORMAT_NAME || manifest.format_version != FORMAT_VERSION {
            return Err(ManifestError::UnsupportedFormat.into());
        }

        let mut paths: HashSet<String> = HashSet::new();
        for artifact in manifest.artifacts.values() {
            validate_artifact_path(&artifact.path)?;
            let key = artifact.path.as_str().to_lowercase();
            if paths.iter().any(|other| paths_conflict(other, &key)) {
                return Err(
                    ManifestError::ConflictingArtifactPath(artifact.path.to_string()).into(),
                );
            }
            paths.insert(key);
        }

        let basis_functions = manifest
            .artifacts
            .get(AO_ERI_ARTIFACT)
            .and_then(|artifact| match &artifact.attributes {
                ArtifactAttributes::AoEri(attributes) => Some(attributes.basis_functions),
                ArtifactAttributes::Unknown(_) => None,
            });

        Ok(Self {
            manifest,
            source: Some(Arc::new(source)),
            context: None,
            request: None,
            sources: Vec::new(),
            staged: BTreeMap::new(),
            basis_functions,
        })
    }

    pub(crate) fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Replaces the AO ERI artifact after checking its compact length.
    ///
    /// # Errors
    ///
    /// Returns an error if basis metadata is missing, the compact ERI length does not match it,
    /// or writing the temporary payload fails.
    pub fn set_eri(&mut self, eri: &CompactEri) -> Result<(), ArtifactError> {
        self.validate_eri(eri)?;
        let directory = tempfile::tempdir().map_err(StorageError::from)?;
        self.stage_eri(eri, directory)
    }

    fn stage_eri(
        &mut self,
        eri: &CompactEri,
        directory: tempfile::TempDir,
    ) -> Result<(), ArtifactError> {
        let mut storage = Storage::folder(directory.path());
        let metadata = storage
            .write_artifact::<ArtifactError, _>(RelativePath::new(AO_ERI_PATH), |writer| {
                write_compact_eri(writer, eri).map_err(Into::into)
            })?;
        self.manifest.artifacts.insert(
            AO_ERI_ARTIFACT.into(),
            ArtifactManifest {
                path: AO_ERI_PATH.into(),
                size: metadata.size,
                digest: metadata.digest,
                representation: COMPACT_ERI_REPRESENTATION.into(),
                attributes: ArtifactAttributes::AoEri(AoEriAttributes {
                    basis_functions: self.basis_functions.ok_or(ArtifactError::Missing)?,
                    computation_version: AO_ERI_COMPUTATION_VERSION,
                }),
            },
        );
        self.staged
            .insert(AO_ERI_ARTIFACT.into(), Arc::new((directory, storage)));
        Ok(())
    }

    pub(super) fn validate_eri(&self, eri: &CompactEri) -> Result<(), ArtifactError> {
        if self
            .context
            .as_ref()
            .is_some_and(|context| context.0.computation_version() != AO_ERI_COMPUTATION_VERSION)
        {
            return Err(ArtifactError::UnsupportedRepresentation(
                "AO ERI computation version".into(),
            ));
        }
        validate_eri_len(eri, self.basis_functions.ok_or(ArtifactError::Missing)?)
    }

    /// Reads and validates the AO ERI without retaining its decoded value.
    ///
    /// # Errors
    /// Returns an error for missing, invalid, corrupt or unreadable artifacts.
    pub fn read_eri(&self) -> Result<CompactEri, ArtifactError> {
        let artifact = self
            .manifest
            .artifacts
            .get(AO_ERI_ARTIFACT)
            .ok_or(ArtifactError::Missing)?;
        let ArtifactAttributes::AoEri(attributes) = &artifact.attributes else {
            return Err(ArtifactError::InvalidMetadata(
                "AO ERI attributes are missing".into(),
            ));
        };
        if artifact.path.as_str() != AO_ERI_PATH
            || artifact.representation != COMPACT_ERI_REPRESENTATION
            || attributes.computation_version != AO_ERI_COMPUTATION_VERSION
        {
            return Err(ArtifactError::UnsupportedRepresentation(
                artifact.representation.clone(),
            ));
        }

        let source = self
            .artifact_source(AO_ERI_ARTIFACT)
            .ok_or(ArtifactError::Missing)?;
        if source.artifact_size(&artifact.path)? != artifact.size {
            return Err(ArtifactError::IntegrityMismatch(artifact.path.to_string()));
        }
        source.with_artifact::<_, ArtifactError, _>(&artifact.path, |reader| {
            let hashing = super::super::checksum::DigestReader::new(reader);
            // Buffer above the hasher so NPY's small reads hash blocks rather than individual values.
            let mut reader = std::io::BufReader::with_capacity(64 * 1024, hashing);
            let value = super::super::npy::read_checked_compact_eri(
                &mut reader,
                attributes.basis_functions,
                artifact.size,
            );
            std::io::copy(&mut reader, &mut std::io::sink()).map_err(StorageError::from)?;
            let (size, digest) = reader.into_inner().finish();
            if size != artifact.size || digest != artifact.digest {
                return Err(ArtifactError::IntegrityMismatch(artifact.path.to_string()));
            }
            value.map_err(Into::into)
        })
    }

    pub(super) fn artifact_source(&self, name: &str) -> Option<&Storage> {
        if let Some(staged) = self.staged.get(name) {
            return Some(&staged.1);
        }
        self.source.as_deref()
    }

    pub(crate) fn write_to(&mut self, destination: Storage) -> Result<(), PersistenceWriteError> {
        self.write_inner(destination, None, true).map(|_| ())
    }

    pub(crate) fn write_storage_with_eri(
        &mut self,
        destination: Storage,
        eri: &CompactEri,
    ) -> Result<(), PersistenceWriteError> {
        self.write_inner(destination, Some(eri), true).map(|_| ())
    }

    pub(crate) fn write_entry_with_eri(
        &mut self,
        destination: Storage,
        eri: &CompactEri,
    ) -> Result<Manifest, PersistenceWriteError> {
        self.write_inner(destination, Some(eri), false)
    }

    pub(crate) fn write_entry(
        &mut self,
        destination: Storage,
    ) -> Result<Manifest, PersistenceWriteError> {
        self.write_inner(destination, None, false)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Artifact publication and manifest ordering form one coherent transaction"
    )]
    fn write_inner(
        &mut self,
        mut destination: Storage,
        eri: Option<&CompactEri>,
        publish_manifest: bool,
    ) -> Result<Manifest, PersistenceWriteError> {
        let mut manifest = self.manifest.clone();
        if manifest.kind == ManifestKind::Portable {
            manifest.producer = Producer {
                name: "RustiQ".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            };
        }

        if let Some(context) = &self.context {
            let metadata = destination.write_json(
                RelativePath::new(super::super::calculation::CALCULATION_PATH),
                &context.0,
            )?;
            manifest.calculation = Some(super::super::manifest::SnapshotManifest {
                path: super::super::calculation::CALCULATION_PATH.into(),
                version: 1,
                size: metadata.size,
                digest: metadata.digest,
            });
        }

        if let Some(request) = &self.request {
            let metadata = destination.write_json(
                RelativePath::new(super::super::request::REQUEST_PATH),
                &super::super::request::RequestSnapshot::from_request(request)
                    .map_err(|error| ArtifactError::InvalidMetadata(error.to_string()))?,
            )?;
            manifest.request = Some(super::super::manifest::SnapshotManifest {
                path: super::super::request::REQUEST_PATH.into(),
                version: 1,
                size: metadata.size,
                digest: metadata.digest,
            });
        }
        manifest.sources.clear();
        for (index, source) in self.sources.iter().enumerate() {
            let path = format!("sources/{index}");
            let metadata = destination.write_artifact::<PersistenceWriteError, _>(
                RelativePath::new(&path),
                |writer| {
                    std::io::Write::write_all(writer, source.bytes())
                        .map_err(StorageError::from)?;
                    Ok(())
                },
            )?;
            manifest
                .sources
                .push(super::super::manifest::SourceManifest {
                    original_name: source.original_name().into(),
                    path,
                    version: 1,
                    size: metadata.size,
                    digest: metadata.digest,
                });
        }

        for (name, artifact) in &self.manifest.artifacts {
            if name == AO_ERI_ARTIFACT && eri.is_some() {
                continue;
            }

            let source = self.artifact_source(name).ok_or_else(|| {
                ArtifactError::InvalidMetadata(format!("artifact {name} has no source"))
            })?;
            validate_artifact_path(&artifact.path)?;

            if source.artifact_size(&artifact.path)? != artifact.size {
                return Err(ArtifactError::IntegrityMismatch(artifact.path.to_string()).into());
            }
            let metadata =
                source.with_artifact::<_, PersistenceWriteError, _>(&artifact.path, |input| {
                    destination.write_artifact::<PersistenceWriteError, _>(
                        &artifact.path,
                        |output| {
                            std::io::copy(input, output)
                                .map_err(StorageError::from)
                                .map_err(PersistenceWriteError::from)?;
                            Ok(())
                        },
                    )
                })?;

            if metadata.size != artifact.size || metadata.digest != artifact.digest {
                return Err(ArtifactError::IntegrityMismatch(artifact.path.to_string()).into());
            }
        }

        if let Some(eri) = eri {
            let basis_functions = self.basis_functions.ok_or(ArtifactError::Missing)?;
            self.validate_eri(eri)?;
            let path = RelativePath::new(AO_ERI_PATH);
            let metadata = destination
                .write_artifact::<PersistenceWriteError, _>(path, |writer| {
                    write_compact_eri(writer, eri).map_err(Into::into)
                })?;

            manifest.artifacts.insert(
                AO_ERI_ARTIFACT.to_owned(),
                ArtifactManifest {
                    path: AO_ERI_PATH.into(),
                    size: metadata.size,
                    representation: COMPACT_ERI_REPRESENTATION.to_owned(),
                    digest: metadata.digest,
                    attributes: ArtifactAttributes::AoEri(AoEriAttributes {
                        basis_functions,
                        computation_version: AO_ERI_COMPUTATION_VERSION,
                    }),
                },
            );
        }

        if publish_manifest {
            destination.write_json(RelativePath::new(MANIFEST_PATH), &manifest)?;
        }
        destination.finish()?;
        Ok(manifest)
    }
}

fn validate_eri_len(eri: &CompactEri, basis_functions: usize) -> Result<(), ArtifactError> {
    let expected = CompactEri::checked_storage_len(basis_functions).ok_or_else(|| {
        ArtifactError::InvalidMetadata("basis-function count overflows compact ERI storage".into())
    })?;
    if eri.len() != expected {
        return Err(ArtifactError::InvalidValueCount {
            basis_functions,
            expected,
            actual: eri.len(),
        });
    }
    Ok(())
}

fn validate_artifact_path(path: &RelativePath) -> Result<(), ArtifactError> {
    validate_storage_path(path).map_err(|_| ArtifactError::InvalidPath(path.to_string()))?;
    if path.as_str().split('/').next() == Some(MANIFEST_PATH) {
        return Err(ArtifactError::InvalidPath(path.to_string()));
    }
    Ok(())
}

fn paths_conflict(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix(right)
            .is_some_and(|suffix| suffix.starts_with('/'))
        || right
            .strip_prefix(left)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::persistence::{
        data::AoEriArtifact, sha256, Sha256Digest, SCIENTIFIC_IDENTITY_VERSION,
    };

    fn new_data() -> RustiQData {
        RustiQData::new_with_identity(
            crate::persistence::ScientificIdentity {
                version: SCIENTIFIC_IDENTITY_VERSION,
                digest: Sha256Digest::from([7; 32]),
            },
            2,
            ManifestKind::Unknown("test-data".to_owned()),
        )
    }

    #[test]
    fn every_read_checks_storage_and_returns_an_owned_value() {
        let root = tempfile::tempdir().unwrap();
        let entry = root.path().join("entry");
        fs::create_dir(&entry).unwrap();
        let mut data = new_data();
        assert!(data.get::<AoEriArtifact>().unwrap().is_none());
        data.set::<AoEriArtifact>(&CompactEri::Zeroed(2)).unwrap();
        data.write_to(Storage::folder(&entry)).unwrap();
        let restored = RustiQData::read_from(Storage::folder(&entry)).unwrap();
        let first = restored.get::<AoEriArtifact>().unwrap().unwrap();
        let second = restored.get::<AoEriArtifact>().unwrap().unwrap();
        assert_eq!(first.ordered_values(), second.ordered_values());
        let path = entry.join(AO_ERI_PATH);
        let mut bytes = fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        fs::write(&path, bytes).unwrap();
        assert!(matches!(
            restored.get::<AoEriArtifact>(),
            Err(ArtifactError::IntegrityMismatch(_))
        ));
        fs::write(&path, b"corrupt").unwrap();
        assert!(matches!(
            restored.get::<AoEriArtifact>(),
            Err(ArtifactError::IntegrityMismatch(_))
        ));
        assert_eq!(first.len(), 6);
    }

    #[test]
    fn set_stages_immediately_and_cleans_up_on_replacement_and_drop() {
        let mut data = new_data();
        data.set_eri(&CompactEri::Zeroed(2)).unwrap();
        let first = data
            .staged
            .get(AO_ERI_ARTIFACT)
            .unwrap()
            .0
            .path()
            .to_path_buf();
        assert!(first.join(AO_ERI_PATH).is_file());
        let before = data.manifest.clone();
        assert!(data.set_eri(&CompactEri::Zeroed(3)).is_err());
        assert_eq!(data.manifest, before);
        assert!(first.exists());
        data.set_eri(&CompactEri::Zeroed(2)).unwrap();
        assert!(!first.exists());
        let second = data
            .staged
            .get(AO_ERI_ARTIFACT)
            .unwrap()
            .0
            .path()
            .to_path_buf();
        assert_eq!(data.read_eri().unwrap().len(), 6);
        drop(data);
        assert!(!second.exists());
    }

    #[test]
    fn clones_share_payload_lifetimes_but_replace_independently() {
        let mut data = new_data();
        data.set_eri(&CompactEri::Zeroed(2)).unwrap();
        let original_path = data.staged[AO_ERI_ARTIFACT].0.path().to_path_buf();
        let copy = data.clone();
        let replacement = CompactEri::from_ordered_values(2, vec![2.0; 6]).unwrap();
        data.set_eri(&replacement).unwrap();
        let replacement_path = data.staged[AO_ERI_ARTIFACT].0.path().to_path_buf();
        assert!(original_path.exists());
        assert_eq!(copy.read_eri().unwrap().ordered_values(), &[0.0; 6]);
        assert_eq!(
            data.read_eri().unwrap().ordered_values(),
            replacement.ordered_values()
        );
        drop(data);
        assert!(!replacement_path.exists());
        assert!(original_path.exists());
        drop(copy);
        assert!(!original_path.exists());
    }

    #[test]
    fn staging_io_failure_keeps_the_previous_payload() {
        let mut data = new_data();
        data.set_eri(&CompactEri::Zeroed(2)).unwrap();
        let before = data.manifest.clone();
        let previous = data
            .staged
            .get(AO_ERI_ARTIFACT)
            .unwrap()
            .0
            .path()
            .to_path_buf();
        let directory = tempfile::tempdir().unwrap();
        let failed = directory.path().to_path_buf();
        fs::write(directory.path().join("arrays"), b"blocks payload directory").unwrap();
        assert!(data.stage_eri(&CompactEri::Zeroed(2), directory).is_err());
        assert!(!failed.exists());
        assert!(previous.exists());
        assert_eq!(data.manifest, before);
        assert_eq!(data.read_eri().unwrap().len(), 6);
    }

    #[test]
    fn unloaded_unknown_artifact_is_copied_without_decoding() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();

        let mut data = new_data();
        data.set_eri(&CompactEri::Zeroed(2)).unwrap();
        data.write_to(Storage::folder(&first)).unwrap();

        let unknown = b"opaque future data";
        let unknown_path = first.join("arrays/post-hf/future.npy");
        fs::create_dir_all(unknown_path.parent().unwrap()).unwrap();
        fs::write(&unknown_path, unknown).unwrap();

        let manifest_path = first.join(MANIFEST_PATH);
        let mut manifest: Manifest =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest.artifacts.insert(
            "future".into(),
            ArtifactManifest {
                path: "arrays/post-hf/future.npy".into(),
                size: unknown.len() as u64,
                representation: "rustiq-future-v1".into(),
                digest: sha256(unknown),
                attributes: ArtifactAttributes::Unknown(BTreeMap::new()),
            },
        );
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();

        let mut restored = RustiQData::read_from(Storage::folder(&first)).unwrap();
        let future_directory = tempfile::tempdir().unwrap();
        let future_root = future_directory.path().to_path_buf();
        let mut future_storage = Storage::folder(&future_root);
        future_storage
            .write_artifact::<StorageError, _>(
                RelativePath::new("arrays/post-hf/future.npy"),
                |writer| {
                    std::io::Write::write_all(writer, unknown)?;
                    Ok(())
                },
            )
            .unwrap();
        restored.staged.insert(
            "future".into(),
            Arc::new((future_directory, future_storage)),
        );
        // Staging or replacing AO ERIs must leave other staged artifacts available.
        restored.set_eri(&CompactEri::Zeroed(2)).unwrap();
        restored.set_eri(&CompactEri::Zeroed(2)).unwrap();
        assert!(future_root.exists());
        restored.write_to(Storage::folder(&second)).unwrap();
        assert_eq!(
            fs::read(second.join("arrays/post-hf/future.npy")).unwrap(),
            unknown
        );
        assert_eq!(
            RustiQData::read_from(Storage::folder(&second))
                .unwrap()
                .manifest,
            manifest
        );
    }

    #[test]
    fn artifact_paths_are_portable_and_relative() {
        assert!(validate_artifact_path(RelativePath::new("arrays/integrals/ao-eri.npy")).is_ok());

        for path in [
            "",
            "/arrays/integrals/ao-eri.npy",
            "arrays\\integrals\\ao-eri.npy",
            "C:/arrays/ao-eri.npy",
            "arrays/../ao-eri.npy",
            "arrays/./ao-eri.npy",
            "arrays//ao-eri.npy",
            "arrays/ao-eri.npy/",
            "manifest.json",
            "manifest.json/child",
        ] {
            assert!(
                validate_artifact_path(RelativePath::new(path)).is_err(),
                "{path} must be rejected"
            );
        }
    }

    #[test]
    fn rejects_corrupt_payload_and_conflicting_paths() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();

        let mut data = new_data();
        data.set_eri(&CompactEri::Zeroed(2)).unwrap();
        data.write_to(Storage::folder(&first)).unwrap();
        fs::write(first.join(AO_ERI_PATH), b"bad").unwrap();

        let mut restored = RustiQData::read_from(Storage::folder(&first)).unwrap();
        assert!(matches!(
            restored.write_to(Storage::folder(&second)),
            Err(PersistenceWriteError::Artifact(
                ArtifactError::IntegrityMismatch(_)
            ))
        ));

        let manifest_path = first.join(MANIFEST_PATH);
        let mut manifest: Manifest =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest.artifacts.insert(
            "future".into(),
            ArtifactManifest {
                path: "arrays/integrals".into(),
                size: 0,
                representation: "future-v1".into(),
                digest: sha256(b""),
                attributes: ArtifactAttributes::Unknown(BTreeMap::new()),
            },
        );
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(matches!(
            RustiQData::read_from(Storage::folder(&first)),
            Err(PersistenceReadError::Manifest(
                ManifestError::ConflictingArtifactPath(_)
            ))
        ));
    }
}
