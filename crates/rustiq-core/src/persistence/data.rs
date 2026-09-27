use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
};

use relative_path::RelativePath;

use crate::{
    basis::Basis, config::validated::PositiveFiniteF64, eri::CompactEri,
    molecules::geometry::Geometry,
};

use super::{
    ao_eri_identity, read_compact_eri, validate_storage_path, write_compact_eri,
    AoEriAttributes, ArtifactAttributes, ArtifactManifest, Manifest, PersistenceError, Producer,
    ScientificIdentityManifest, Storage, AO_ERI_COMPUTATION_VERSION, AO_ERI_PATH,
    COMPACT_ERI_REPRESENTATION, FORMAT_NAME, FORMAT_VERSION, MANIFEST_PATH,
};

pub(crate) const AO_ERI_ARTIFACT: &str = "ao_eri";
pub(crate) const CACHE_KIND: &str = "integral-cache";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

mod private {
    pub trait Sealed {}
}

/// A known scientific artifact. Only RustiQ's declared artifact markers implement this trait.
pub trait Artifact: private::Sealed {
    type Value;

    #[doc(hidden)]
    fn get(data: &mut RustiQData) -> Result<Option<&Self::Value>, PersistenceError>;
    #[doc(hidden)]
    fn set(data: &mut RustiQData, value: Self::Value) -> Result<(), PersistenceError>;
}

/// AO electron-repulsion integrals in compact storage order.
pub struct AoEriArtifact;

impl private::Sealed for AoEriArtifact {}

impl Artifact for AoEriArtifact {
    type Value = CompactEri;

    fn get(data: &mut RustiQData) -> Result<Option<&CompactEri>, PersistenceError> {
        if data.ao_eri.is_some() {
            return Ok(data.ao_eri.as_ref());
        }
        if !data.manifest.artifacts.contains_key(AO_ERI_ARTIFACT) {
            return Ok(None);
        }
        data.read_eri().map(Some)
    }

    fn set(data: &mut RustiQData, value: CompactEri) -> Result<(), PersistenceError> {
        data.set_eri(value)
    }
}

/// Known scientific artifacts and their manifest, with NPY values loaded on demand.
///
/// Add a typed field and accessors here when RustiQ gains a new scientific artifact.
/// Unknown manifest entries remain opaque and are copied when writing a new entry.
#[derive(Debug)]
pub struct RustiQData {
    manifest: Manifest,
    source: Option<Storage>,
    ao_eri: Option<CompactEri>,
    basis_functions: Option<usize>,
}

impl RustiQData {
    /// Gets a known artifact, loading and caching its value on first access.
    pub fn get<A: Artifact>(&mut self) -> Result<Option<&A::Value>, PersistenceError> {
        A::get(self)
    }

    /// Sets a known artifact using its statically selected value type.
    pub fn set<A: Artifact>(&mut self, value: A::Value) -> Result<(), PersistenceError> {
        A::set(self, value)
    }

    /// Starts a new AO ERI entry with the same scientific identity as the cache.
    pub fn new(geometry: &Geometry, basis: &Basis, threshold: Option<PositiveFiniteF64>) -> Self {
        let identity = ao_eri_identity(geometry, basis, threshold);
        Self::new_with_identity(identity, basis.nbasis())
    }

    pub(crate) fn new_with_identity(
        identity: super::ScientificIdentity,
        basis_functions: usize,
    ) -> Self {
        Self {
            manifest: Manifest {
                format: FORMAT_NAME.to_owned(),
                format_version: FORMAT_VERSION,
                kind: CACHE_KIND.to_owned(),
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

    /// Reads only the bounded manifest. NPY values are opened on demand.
    pub fn read(directory: impl AsRef<Path>) -> Result<Self, PersistenceError> {
        let directory = directory.as_ref();
        let metadata = fs::symlink_metadata(directory)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(PersistenceError::InvalidManifest(
                "entry is not a regular directory".into(),
            ));
        }
        let source = Storage::folder(fs::canonicalize(directory)?);
        let manifest: Manifest =
            source.read_json(RelativePath::new(MANIFEST_PATH), MAX_MANIFEST_BYTES)?;
        if manifest.format != FORMAT_NAME || manifest.format_version != FORMAT_VERSION {
            return Err(PersistenceError::InvalidManifest(
                "unsupported format or version".into(),
            ));
        }
        let mut paths: HashSet<&str> = HashSet::new();
        for artifact in manifest.artifacts.values() {
            validate_artifact_path(&artifact.path)?;
            if paths
                .iter()
                .any(|other| paths_conflict(other, artifact.path.as_str()))
            {
                return Err(PersistenceError::InvalidManifest(format!(
                    "artifact path conflicts with another artifact: {}",
                    artifact.path
                )));
            }
            paths.insert(artifact.path.as_str());
        }
        let basis_functions = manifest
            .artifacts
            .get(AO_ERI_ARTIFACT)
            .and_then(|artifact| {
                if let ArtifactAttributes::AoEri(attributes) = &artifact.attributes {
                    Some(attributes.basis_functions)
                } else {
                    None
                }
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
    pub fn set_eri(&mut self, eri: CompactEri) -> Result<(), PersistenceError> {
        let basis_functions = self.basis_functions.ok_or(PersistenceError::MissingEri)?;
        validate_eri_len(&eri, basis_functions)?;
        self.ao_eri = Some(eri);
        Ok(())
    }

    /// Validates and decodes the AO ERI NPY on first access, then reuses the object.
    pub fn read_eri(&mut self) -> Result<&CompactEri, PersistenceError> {
        if self.ao_eri.is_none() {
            let artifact = self
                .manifest
                .artifacts
                .get(AO_ERI_ARTIFACT)
                .ok_or(PersistenceError::MissingEri)?;
            let ArtifactAttributes::AoEri(attributes) = &artifact.attributes else {
                return Err(PersistenceError::InvalidArtifact(
                    "AO ERI attributes are missing".into(),
                ));
            };
            if artifact.path.as_str() != AO_ERI_PATH
                || artifact.representation != COMPACT_ERI_REPRESENTATION
                || attributes.computation_version != AO_ERI_COMPUTATION_VERSION
            {
                return Err(PersistenceError::InvalidArtifact(
                    "unsupported AO ERI representation".into(),
                ));
            }
            let source = self.source.as_ref().ok_or(PersistenceError::MissingEri)?;
            let metadata = source.artifact_metadata(&artifact.path)?;
            if metadata.size != artifact.size || metadata.digest != artifact.digest {
                return Err(PersistenceError::InvalidArtifact(
                    "AO ERI digest or size mismatch".into(),
                ));
            }
            self.ao_eri = Some(source.with_artifact(&artifact.path, |reader| {
                read_compact_eri(reader, attributes.basis_functions)
            })?);
        }
        Ok(self
            .ao_eri
            .as_ref()
            .expect("ERI was loaded or already present"))
    }

    pub(crate) fn take_eri(&mut self) -> Option<CompactEri> {
        self.ao_eri.take()
    }

    /// Writes to a new directory, copying unloaded artifacts without decoding them.
    pub fn write(&self, directory: impl AsRef<Path>) -> Result<(), PersistenceError> {
        self.write_inner(directory.as_ref(), self.ao_eri.as_ref())
    }

    pub(crate) fn write_with_eri(
        &self,
        directory: impl AsRef<Path>,
        eri: Option<&CompactEri>,
    ) -> Result<(), PersistenceError> {
        self.write_inner(directory.as_ref(), eri)
    }

    fn write_inner(
        &self,
        directory: &Path,
        eri: Option<&CompactEri>,
    ) -> Result<(), PersistenceError> {
        if directory.exists() {
            return Err(PersistenceError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "persistence destination already exists",
            )));
        }
        let parent = directory
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let staging = tempfile::Builder::new()
            .prefix(".rustiq-data-")
            .tempdir_in(parent)?;
        let mut manifest = self.manifest.clone();
        let mut destination = Storage::folder(staging.path());

        for (name, artifact) in &self.manifest.artifacts {
            if name == AO_ERI_ARTIFACT && eri.is_some() {
                continue;
            }
            let source = self.source.as_ref().ok_or_else(|| {
                PersistenceError::InvalidArtifact(format!("artifact {name} has no source file"))
            })?;
            validate_artifact_path(&artifact.path)?;
            let metadata = source.with_artifact(&artifact.path, |input| {
                destination.write_artifact(&artifact.path, |output| {
                    std::io::copy(input, output)?;
                    Ok(())
                })
            })?;
            if metadata.size != artifact.size || metadata.digest != artifact.digest {
                return Err(PersistenceError::InvalidArtifact(format!(
                    "artifact digest or size mismatch: {}",
                    artifact.path
                )));
            }
        }

        if let Some(eri) = eri {
            let basis_functions = self.basis_functions.ok_or(PersistenceError::MissingEri)?;
            validate_eri_len(eri, basis_functions)?;
            let path = RelativePath::new(AO_ERI_PATH);
            let metadata =
                destination.write_artifact(path, |writer| write_compact_eri(writer, eri))?;
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
        if manifest.kind == CACHE_KIND && !manifest.artifacts.contains_key(AO_ERI_ARTIFACT) {
            return Err(PersistenceError::MissingEri);
        }
        destination.write_json(RelativePath::new(MANIFEST_PATH), &manifest)?;
        fs::rename(staging.path(), directory)?;
        Ok(())
    }
}

fn validate_eri_len(eri: &CompactEri, basis_functions: usize) -> Result<(), PersistenceError> {
    let expected = CompactEri::checked_storage_len(basis_functions).ok_or_else(|| {
        PersistenceError::InvalidArtifact(
            "basis-function count overflows compact ERI storage".into(),
        )
    })?;
    if eri.len() != expected {
        return Err(PersistenceError::InvalidValueCount {
            basis_functions,
            expected,
            actual: eri.len(),
        });
    }
    Ok(())
}

fn validate_artifact_path(path: &RelativePath) -> Result<(), PersistenceError> {
    validate_storage_path(path)?;
    if path.as_str().split('/').next() == Some(MANIFEST_PATH) {
        return Err(PersistenceError::InvalidArtifact(format!(
            "unsafe artifact path: {path}"
        )));
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
    use super::*;
    use crate::persistence::{sha256, Sha256Digest, SCIENTIFIC_IDENTITY_VERSION};

    fn new_data() -> RustiQData {
        RustiQData::new_with_identity(
            super::super::ScientificIdentity {
                version: SCIENTIFIC_IDENTITY_VERSION,
                digest: Sha256Digest::from([7; 32]),
            },
            2,
        )
    }

    #[test]
    fn eri_is_loaded_only_on_request_and_then_cached_as_an_object() {
        let root = tempfile::tempdir().unwrap();
        let entry = root.path().join("entry");
        let mut data = new_data();
        assert!(data.get::<AoEriArtifact>().unwrap().is_none());
        data.set::<AoEriArtifact>(CompactEri::Zeroed(2)).unwrap();
        data.write(&entry).unwrap();
        let mut restored = RustiQData::read(&entry).unwrap();
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
        let mut data = new_data();
        data.set_eri(CompactEri::Zeroed(2)).unwrap();
        data.write(&first).unwrap();
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
        let restored = RustiQData::read(&first).unwrap();
        assert!(restored.ao_eri.is_none());
        restored.write(&second).unwrap();
        assert_eq!(
            fs::read(second.join("arrays/post-hf/future.npy")).unwrap(),
            unknown
        );
        assert_eq!(RustiQData::read(&second).unwrap().manifest, manifest);
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
        let mut data = new_data();
        data.set_eri(CompactEri::Zeroed(2)).unwrap();
        data.write(&first).unwrap();
        fs::write(first.join(AO_ERI_PATH), b"bad").unwrap();
        assert!(matches!(
            RustiQData::read(&first).unwrap().write(&second),
            Err(PersistenceError::InvalidArtifact(_))
        ));
        assert!(!second.exists());

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
            RustiQData::read(&first),
            Err(PersistenceError::InvalidManifest(_))
        ));
    }
}
