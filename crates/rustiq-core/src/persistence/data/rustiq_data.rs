use std::collections::{BTreeMap, HashSet};

use relative_path::RelativePath;

use crate::{
    basis::Basis, config::validated::PositiveFiniteF64, eri::CompactEri,
    molecules::geometry::Geometry,
};

use super::super::{
    ao_eri_identity, read_compact_eri, validate_storage_path, write_compact_eri, AoEriAttributes,
    ArtifactAttributes, ArtifactError, ArtifactManifest, Manifest, ManifestError, ManifestKind,
    PersistenceReadError, PersistenceWriteError, Producer, ScientificIdentityManifest, Storage,
    StorageError, AO_ERI_COMPUTATION_VERSION, AO_ERI_PATH, COMPACT_ERI_REPRESENTATION, FORMAT_NAME,
    FORMAT_VERSION, MANIFEST_PATH,
};
use super::artifact::Artifact;

pub(crate) const AO_ERI_ARTIFACT: &str = "ao_eri";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

/// Known scientific artifacts and their manifest, with values loaded on demand.
#[derive(Debug)]
pub struct RustiQData {
    pub(super) manifest: Manifest,
    source: Option<Storage>,
    pub(super) ao_eri: Option<CompactEri>,
    basis_functions: Option<usize>,
}

impl RustiQData {
    /// Gets a known scientific artifact, loading and caching it on first access.
    pub fn get<A: Artifact>(&mut self) -> Result<Option<&A::Value>, ArtifactError> {
        A::get(self)
    }

    /// Sets a known scientific artifact using its statically selected value type.
    pub fn set<A: Artifact>(&mut self, value: A::Value) -> Result<(), ArtifactError> {
        A::set(self, value)
    }

    /// Starts a new checkpoint data set with the current scientific identity.
    pub fn new(geometry: &Geometry, basis: &Basis, threshold: Option<PositiveFiniteF64>) -> Self {
        let identity = ao_eri_identity(geometry, basis, threshold);
        Self::new_with_identity(identity, basis.nbasis(), ManifestKind::Checkpoint)
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
            },
            source: None,
            ao_eri: None,
            basis_functions: Some(basis_functions),
        }
    }

    pub(crate) fn read_from(mut source: Storage) -> Result<Self, PersistenceReadError> {
        let manifest: Manifest =
            source.read_json(RelativePath::new(MANIFEST_PATH), MAX_MANIFEST_BYTES)?;
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
            source: Some(source),
            ao_eri: None,
            basis_functions,
        })
    }

    pub(crate) fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Replaces the AO ERI artifact after checking its compact length.
    pub fn set_eri(&mut self, eri: CompactEri) -> Result<(), ArtifactError> {
        let basis_functions = self.basis_functions.ok_or(ArtifactError::Missing)?;
        validate_eri_len(&eri, basis_functions)?;
        self.ao_eri = Some(eri);
        Ok(())
    }

    /// Validates and decodes the AO ERI on first access, then reuses the object.
    pub fn read_eri(&mut self) -> Result<&CompactEri, ArtifactError> {
        if self.ao_eri.is_none() {
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

            let source = self.source.as_mut().ok_or(ArtifactError::Missing)?;
            let metadata = source.artifact_metadata(&artifact.path)?;
            if metadata.size != artifact.size || metadata.digest != artifact.digest {
                return Err(ArtifactError::IntegrityMismatch(artifact.path.to_string()));
            }

            self.ao_eri = Some(
                source.with_artifact::<_, ArtifactError, _>(&artifact.path, |reader| {
                    read_compact_eri(reader, attributes.basis_functions).map_err(Into::into)
                })?,
            );
        }

        Ok(self
            .ao_eri
            .as_ref()
            .expect("ERI was loaded or already present"))
    }

    pub(crate) fn take_eri(&mut self) -> Option<CompactEri> {
        self.ao_eri.take()
    }

    pub(crate) fn write_to(&mut self, destination: Storage) -> Result<(), PersistenceWriteError> {
        let eri = self.ao_eri.take();
        let result = self.write_inner(destination, eri.as_ref());
        self.ao_eri = eri;
        result
    }

    pub(crate) fn write_with_eri(
        &mut self,
        destination: Storage,
        eri: &CompactEri,
    ) -> Result<(), PersistenceWriteError> {
        self.write_inner(destination, Some(eri))
    }

    fn write_inner(
        &mut self,
        mut destination: Storage,
        eri: Option<&CompactEri>,
    ) -> Result<(), PersistenceWriteError> {
        let mut manifest = self.manifest.clone();

        for (name, artifact) in &self.manifest.artifacts {
            if name == AO_ERI_ARTIFACT && eri.is_some() {
                continue;
            }

            let source = self.source.as_mut().ok_or_else(|| {
                ArtifactError::InvalidMetadata(format!("artifact {name} has no source"))
            })?;
            validate_artifact_path(&artifact.path)?;

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
            validate_eri_len(eri, basis_functions)?;
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

        destination.write_json(RelativePath::new(MANIFEST_PATH), &manifest)?;
        destination.finish()?;
        Ok(())
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
            ManifestKind::Checkpoint,
        )
    }

    #[test]
    fn checkpoint_can_be_written_without_ao_eri() {
        let root = tempfile::tempdir().unwrap();
        let entry = root.path().join("checkpoint");
        fs::create_dir(&entry).unwrap();

        let mut data = new_data();
        data.write_to(Storage::folder(&entry)).unwrap();

        let restored = RustiQData::read_from(Storage::folder(&entry)).unwrap();
        assert_eq!(restored.manifest().kind, ManifestKind::Checkpoint);
        assert!(!restored.manifest().artifacts.contains_key(AO_ERI_ARTIFACT));
    }

    #[test]
    fn eri_is_loaded_only_on_request_and_then_cached_as_an_object() {
        let root = tempfile::tempdir().unwrap();
        let entry = root.path().join("entry");
        fs::create_dir(&entry).unwrap();
        let mut data = new_data();
        assert!(data.get::<AoEriArtifact>().unwrap().is_none());
        data.set::<AoEriArtifact>(CompactEri::Zeroed(2)).unwrap();
        data.write_to(Storage::folder(&entry)).unwrap();

        let mut restored = RustiQData::read_from(Storage::folder(&entry)).unwrap();
        assert!(restored.ao_eri.is_none());
        let first = restored.get::<AoEriArtifact>().unwrap().unwrap() as *const CompactEri;
        fs::remove_file(entry.join(AO_ERI_PATH)).unwrap();
        let second = restored.get::<AoEriArtifact>().unwrap().unwrap() as *const CompactEri;
        assert_eq!(first, second);
        assert_eq!(restored.read_eri().unwrap().len(), 6);
    }

    #[test]
    fn unloaded_unknown_artifact_is_copied_without_decoding() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();

        let mut data = new_data();
        data.set_eri(CompactEri::Zeroed(2)).unwrap();
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
        data.set_eri(CompactEri::Zeroed(2)).unwrap();
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
