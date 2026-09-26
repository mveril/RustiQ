use std::{
    fs::{self, File},
    io::{self, Seek},
    path::{Path, PathBuf},
};

use tempfile::Builder;

use crate::{
    basis::Basis, config::validated::PositiveFiniteF64, eri::CompactEri,
    molecules::molecule::Molecule,
};

use super::{
    ao_eri_identity, sha256_reader, validate_compact_eri_header, AoEriAttributes,
    ArtifactAttributes, ArtifactManifest, Manifest, RustiQData, ScientificIdentity,
    AO_ERI_COMPUTATION_VERSION, AO_ERI_PATH, COMPACT_ERI_REPRESENTATION, FORMAT_NAME,
    FORMAT_VERSION, SCIENTIFIC_IDENTITY_VERSION,
};

use super::data::{AO_ERI_ARTIFACT, CACHE_KIND};

/// A directory-backed AO ERI cache entry available for management.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EriCacheEntry {
    /// Persisted human-readable alias, when available.
    pub name: Option<String>,
    pub fingerprint: String,
    pub payload_size: Option<u64>,
    pub verified: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EriCacheReference {
    pub(crate) fingerprint: String,
    pub(crate) name: Option<String>,
}

/// Directory-backed cache for deterministic AO electron-repulsion integrals.
///
/// The caller selects `root`; the scientific core never infers a cache location.
/// Entries are addressed by their effective scientific identity and are immutable
/// once published.
#[derive(Clone, Debug)]
pub struct EriCache {
    root: PathBuf,
}

impl EriCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Lists the published ERI cache entries without following symbolic links.
    pub fn entries(&self) -> io::Result<Vec<EriCacheEntry>> {
        let names = super::cache_names::mappings(&self.root).unwrap_or_default();
        let mut result = Vec::new();
        for fingerprint in self.fingerprints()? {
            let entry_path = self.root.join("eri").join(&fingerprint);
            let manifest = read_manifest(&entry_path);
            let artifact = manifest
                .as_ref()
                .and_then(|manifest| manifest.artifacts.get(AO_ERI_ARTIFACT));
            let verified = manifest.as_ref().is_some_and(|manifest| {
                manifest_is_valid(
                    manifest,
                    ScientificIdentity {
                        version: SCIENTIFIC_IDENTITY_VERSION,
                        digest: manifest.scientific_identity.digest,
                    },
                ) && manifest.scientific_identity.digest.to_hex() == fingerprint
                    && artifact.is_some_and(|artifact| {
                        ao_eri_attributes(artifact).is_some_and(|attributes| {
                            validate_payload(&entry_path, artifact, attributes.basis_functions)
                        })
                    })
            });
            result.push(EriCacheEntry {
                name: names
                    .iter()
                    .find(|(_, value)| **value == fingerprint)
                    .map(|(name, _)| name.clone()),
                fingerprint,
                payload_size: artifact.map(|artifact| artifact.size),
                verified,
            });
        }
        Ok(result)
    }

    /// Lists published directory fingerprints without inspecting their payloads.
    fn fingerprints(&self) -> io::Result<Vec<String>> {
        let parent = self.root.join("eri");
        match super::cache_names::regular_directory(&parent) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            result => result?,
        }
        let entries = match fs::read_dir(parent) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let mut result = Vec::new();
        for entry in entries {
            let entry = entry?;
            let fingerprint = entry.file_name().to_string_lossy().into_owned();
            let metadata = fs::symlink_metadata(entry.path())?;
            if !is_fingerprint(&fingerprint)
                || !metadata.is_dir()
                || metadata.file_type().is_symlink()
            {
                continue;
            }
            result.push(fingerprint);
        }
        result.sort();
        Ok(result)
    }

    /// Assigns persistent aliases to published entries without reading payloads.
    /// Failure leaves all scientific entries intact; callers may still list them.
    pub fn assign_missing_names(&self) -> io::Result<()> {
        let mut names = super::cache_names::mappings(&self.root)?;
        for fingerprint in self.fingerprints()? {
            let name = super::cache_names::assign(&self.root, &fingerprint, &names)?;
            names.insert(name, fingerprint);
        }
        Ok(())
    }

    /// Resolves a persistent alias to a published fingerprint without reading NPY.
    pub fn resolve_name(&self, name: &str) -> io::Result<Option<String>> {
        let Some(fingerprint) = super::cache_names::resolve(&self.root, name)? else {
            return Ok(None);
        };
        match super::cache_names::regular_directory(&self.root.join("eri")) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            result => result?,
        }
        match super::cache_names::regular_directory(&self.root.join("eri").join(&fingerprint)) {
            Ok(()) => Ok(Some(fingerprint)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Removes an entry by its full fingerprint or persistent alias.
    pub fn remove_named(&self, target: &str) -> io::Result<bool> {
        if is_fingerprint(target) {
            return self.remove(target);
        }
        match self.resolve_name(target)? {
            Some(fingerprint) => self.remove(&fingerprint),
            None => Ok(false),
        }
    }

    /// Removes exactly one published entry identified by its full SHA-256 hex digest.
    ///
    /// Returns `false` when the entry does not exist. Symbolic links and invalid
    /// fingerprints are rejected rather than being followed or interpreted as paths.
    pub fn remove(&self, fingerprint: &str) -> io::Result<bool> {
        if !is_fingerprint(fingerprint) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ERI cache fingerprint must be 64 lowercase hexadecimal characters",
            ));
        }
        let entry = self.root.join("eri").join(fingerprint);
        match super::cache_names::regular_directory(&self.root.join("eri")) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            result => result?,
        }
        let metadata = match fs::symlink_metadata(&entry) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ERI cache entry is not a regular directory",
            ));
        }
        fs::remove_dir_all(entry)?;
        if let Ok(names) = super::cache_names::mappings(&self.root) {
            for (name, value) in names {
                if value == fingerprint {
                    let _ = super::cache_names::remove_alias(&self.root, &name);
                }
            }
        }
        Ok(true)
    }

    /// Removes every published ERI cache entry below this cache root.
    pub fn remove_all(&self) -> io::Result<()> {
        for fingerprint in self.fingerprints()? {
            self.remove(&fingerprint)?;
        }
        if let Ok(names) = super::cache_names::mappings(&self.root) {
            for (name, fingerprint) in names {
                if fs::symlink_metadata(self.root.join("eri").join(fingerprint))
                    .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
                {
                    let _ = super::cache_names::remove_alias(&self.root, &name);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn load(
        &self,
        molecule: &Molecule,
        basis: &Basis,
        threshold: Option<PositiveFiniteF64>,
    ) -> Option<CompactEri> {
        self.load_with_reference(molecule, basis, threshold)
            .map(|(eri, _)| eri)
    }

    pub(crate) fn load_with_reference(
        &self,
        molecule: &Molecule,
        basis: &Basis,
        threshold: Option<PositiveFiniteF64>,
    ) -> Option<(CompactEri, EriCacheReference)> {
        let identity = ao_eri_identity(molecule.geometry(), basis, threshold);
        self.load_identity(identity, basis.nbasis())
            .map(|eri| (eri, self.reference(identity)))
    }

    fn reference(&self, identity: ScientificIdentity) -> EriCacheReference {
        let fingerprint = identity.digest.to_hex();
        let name = super::cache_names::mappings(&self.root)
            .ok()
            .and_then(|names| {
                names
                    .into_iter()
                    .find(|(_, value)| value == &fingerprint)
                    .map(|(name, _)| name)
            })
            .or_else(|| {
                super::cache_names::assign(
                    &self.root,
                    &fingerprint,
                    &std::collections::BTreeMap::new(),
                )
                .ok()
            });
        EriCacheReference { fingerprint, name }
    }

    pub(crate) fn store_with_reference(
        &self,
        molecule: &Molecule,
        basis: &Basis,
        threshold: Option<PositiveFiniteF64>,
        eri: &CompactEri,
    ) -> io::Result<EriCacheReference> {
        let identity = ao_eri_identity(molecule.geometry(), basis, threshold);
        self.store_identity(identity, basis.nbasis(), eri)?;
        let fingerprint = identity.digest.to_hex();
        let name = super::cache_names::mappings(&self.root)
            .ok()
            .and_then(|names| {
                names
                    .into_iter()
                    .find(|(_, value)| value == &fingerprint)
                    .map(|(name, _)| name)
            })
            .or_else(|| {
                super::cache_names::assign(
                    &self.root,
                    &fingerprint,
                    &std::collections::BTreeMap::new(),
                )
                .ok()
            });
        Ok(EriCacheReference { fingerprint, name })
    }

    pub(crate) fn store(
        &self,
        molecule: &Molecule,
        basis: &Basis,
        threshold: Option<PositiveFiniteF64>,
        eri: &CompactEri,
    ) -> io::Result<()> {
        let _ = self.store_with_reference(molecule, basis, threshold, eri)?;
        Ok(())
    }

    fn entry_path(&self, identity: ScientificIdentity) -> PathBuf {
        self.root.join("eri").join(identity.digest.to_hex())
    }

    fn load_identity(
        &self,
        identity: ScientificIdentity,
        basis_functions: usize,
    ) -> Option<CompactEri> {
        let entry = self.entry_path(identity);
        let metadata = fs::symlink_metadata(&entry).ok()?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return None;
        }
        let mut data = RustiQData::read(&entry).ok()?;
        let manifest = data.manifest();
        let artifact = manifest.artifacts.get(AO_ERI_ARTIFACT)?;
        let attributes = ao_eri_attributes(artifact)?;
        if !manifest_is_valid(manifest, identity) || attributes.basis_functions != basis_functions {
            return None;
        }
        data.read_eri().ok()?;
        data.take_eri()
    }

    fn store_identity(
        &self,
        identity: ScientificIdentity,
        basis_functions: usize,
        eri: &CompactEri,
    ) -> io::Result<()> {
        let final_entry = self.entry_path(identity);
        let parent = final_entry.parent().expect("ERI cache entry has a parent");
        match super::cache_names::regular_directory(parent) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if self.load_identity(identity, basis_functions).is_some() {
            return Ok(());
        }
        fs::create_dir_all(parent)?;
        let temporary = Builder::new().prefix(".rustiq-eri-").tempdir_in(parent)?;
        let entry_data = RustiQData::new_with_identity(identity, basis_functions);
        let staged_entry = temporary.path().join("entry");
        entry_data
            .write_with_eri(&staged_entry, Some(eri))
            .map_err(io::Error::other)?;
        let temporary_path = temporary.keep();
        let staged_entry = temporary_path.join("entry");
        if fs::symlink_metadata(&final_entry).is_ok() {
            if self.load_identity(identity, basis_functions).is_some() {
                let _ = fs::remove_dir_all(temporary_path);
                return Ok(());
            }
            if let Err(error) = remove_invalid_entry(&final_entry) {
                let _ = fs::remove_dir_all(&temporary_path);
                return Err(error);
            }
        }
        match fs::rename(&staged_entry, &final_entry) {
            Ok(()) => {
                let _ = fs::remove_dir_all(temporary_path);
                Ok(())
            }
            Err(error) => {
                let _ = fs::remove_dir_all(temporary_path);
                if self.load_identity(identity, basis_functions).is_some() {
                    Ok(())
                } else {
                    Err(error)
                }
            }
        }
    }
}

fn read_manifest(entry: &Path) -> Option<Manifest> {
    RustiQData::read(entry)
        .ok()
        .map(|data| data.manifest().clone())
}

fn manifest_is_valid(manifest: &Manifest, identity: ScientificIdentity) -> bool {
    manifest.format == FORMAT_NAME
        && manifest.format_version == FORMAT_VERSION
        && manifest.kind == CACHE_KIND
        && manifest.scientific_identity.version == identity.version
        && manifest.scientific_identity.digest == identity.digest
        && manifest
            .artifacts
            .get(AO_ERI_ARTIFACT)
            .is_some_and(|artifact| {
                artifact.path == AO_ERI_PATH
                    && artifact.representation == COMPACT_ERI_REPRESENTATION
                    && ao_eri_attributes(artifact).is_some_and(|attributes| {
                        attributes.computation_version == AO_ERI_COMPUTATION_VERSION
                    })
            })
}

fn ao_eri_attributes(artifact: &ArtifactManifest) -> Option<&AoEriAttributes> {
    match &artifact.attributes {
        ArtifactAttributes::AoEri(attributes) => Some(attributes),
        ArtifactAttributes::Unknown(_) => None,
    }
}

fn validate_payload(entry: &Path, artifact: &ArtifactManifest, basis_functions: usize) -> bool {
    if CompactEri::checked_storage_len(basis_functions).is_none() {
        return false;
    }
    let file_path = entry.join(AO_ERI_PATH);
    let metadata = match fs::symlink_metadata(&file_path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return false;
    }
    let mut file = match File::open(&file_path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    if file
        .metadata()
        .ok()
        .is_none_or(|metadata| metadata.len() != artifact.size)
    {
        return false;
    }
    if sha256_reader(&mut file).ok() != Some(artifact.digest) {
        return false;
    }
    if file.rewind().is_err() {
        return false;
    }
    validate_compact_eri_header(file, basis_functions, artifact.size)
}

fn remove_invalid_entry(entry: &Path) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(entry) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(entry)
    } else {
        fs::remove_file(entry)
    }
}

pub(super) fn is_fingerprint(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::MANIFEST_PATH;
    use crate::{config::DEFAULT_ERI_SCHWARZ_THRESHOLD, test_utils::load_sto3g_basis};

    fn input() -> (Molecule, Basis) {
        let geometry = crate::molecules::geometry::Geometry::from_source(
            "cache-test.xyz",
            "2\nH2\nH 0 0 0\nH 1.4 0 0\n",
        )
        .unwrap();
        let molecule = Molecule::try_new(
            geometry,
            crate::molecules::units::Units::Bohr,
            0,
            std::num::NonZero::new(1).unwrap(),
        )
        .unwrap();
        let basis = load_sto3g_basis(molecule.geometry());
        (molecule, basis)
    }
    fn threshold() -> Option<PositiveFiniteF64> {
        Some(PositiveFiniteF64::try_new(DEFAULT_ERI_SCHWARZ_THRESHOLD).unwrap())
    }

    #[test]
    fn stores_and_loads_a_validated_entry() {
        let temporary = tempfile::tempdir().unwrap();
        let cache = EriCache::new(temporary.path());
        let (molecule, basis) = input();
        let eri = CompactEri::Zeroed(basis.nbasis());
        cache.store(&molecule, &basis, threshold(), &eri).unwrap();
        assert_eq!(
            cache
                .load(&molecule, &basis, threshold())
                .unwrap()
                .ordered_values(),
            eri.ordered_values()
        );
    }
    #[test]
    fn corruption_is_a_cache_miss() {
        let temporary = tempfile::tempdir().unwrap();
        let cache = EriCache::new(temporary.path());
        let (molecule, basis) = input();
        cache
            .store(
                &molecule,
                &basis,
                threshold(),
                &CompactEri::Zeroed(basis.nbasis()),
            )
            .unwrap();
        let identity = ao_eri_identity(molecule.geometry(), &basis, threshold());
        fs::write(cache.entry_path(identity).join(AO_ERI_PATH), b"corrupt").unwrap();
        assert!(cache.load(&molecule, &basis, threshold()).is_none());
    }

    #[test]
    fn corrupted_entry_is_repaired_when_storing() {
        let temporary = tempfile::tempdir().unwrap();
        let cache = EriCache::new(temporary.path());
        let (molecule, basis) = input();
        let eri = CompactEri::Zeroed(basis.nbasis());
        cache.store(&molecule, &basis, threshold(), &eri).unwrap();
        let identity = ao_eri_identity(molecule.geometry(), &basis, threshold());
        fs::write(cache.entry_path(identity).join(AO_ERI_PATH), b"corrupt").unwrap();

        cache.store(&molecule, &basis, threshold(), &eri).unwrap();

        assert_eq!(
            cache
                .load(&molecule, &basis, threshold())
                .unwrap()
                .ordered_values(),
            eri.ordered_values()
        );
    }

    #[test]
    fn aliases_are_management_metadata_only() {
        let temporary = tempfile::tempdir().unwrap();
        let cache = EriCache::new(temporary.path());
        let (molecule, basis) = input();
        let eri = CompactEri::Zeroed(basis.nbasis());
        cache.store(&molecule, &basis, threshold(), &eri).unwrap();

        let identity = ao_eri_identity(molecule.geometry(), &basis, threshold());
        let entry = cache.entry_path(identity);
        let manifest_before = fs::read(entry.join(MANIFEST_PATH)).unwrap();
        let payload_before = fs::read(entry.join(AO_ERI_PATH)).unwrap();
        let listed = cache.entries().unwrap();
        let name = listed[0].name.clone().unwrap();

        assert_eq!(
            cache.resolve_name(&name).unwrap(),
            Some(identity.digest.to_hex())
        );
        assert_eq!(
            cache
                .load(&molecule, &basis, threshold())
                .unwrap()
                .ordered_values(),
            eri.ordered_values()
        );
        assert_eq!(
            fs::read(entry.join(MANIFEST_PATH)).unwrap(),
            manifest_before
        );
        assert_eq!(fs::read(entry.join(AO_ERI_PATH)).unwrap(), payload_before);
    }

    #[test]
    fn entries_mark_corrupted_payload_as_invalid() {
        let temporary = tempfile::tempdir().unwrap();
        let cache = EriCache::new(temporary.path());
        let (molecule, basis) = input();
        cache
            .store(
                &molecule,
                &basis,
                threshold(),
                &CompactEri::Zeroed(basis.nbasis()),
            )
            .unwrap();
        let identity = ao_eri_identity(molecule.geometry(), &basis, threshold());
        fs::write(cache.entry_path(identity).join(AO_ERI_PATH), b"corrupt").unwrap();

        let entries = cache.entries().unwrap();

        assert_eq!(entries.len(), 1);
        assert!(!entries[0].verified);
    }

    #[test]
    fn stale_eri_computation_version_is_invalid_and_not_loaded() {
        let temporary = tempfile::tempdir().unwrap();
        let cache = EriCache::new(temporary.path());
        let (molecule, basis) = input();
        cache
            .store(
                &molecule,
                &basis,
                threshold(),
                &CompactEri::Zeroed(basis.nbasis()),
            )
            .unwrap();
        let identity = ao_eri_identity(molecule.geometry(), &basis, threshold());
        let manifest_path = cache.entry_path(identity).join(MANIFEST_PATH);
        let mut manifest: serde_json::Value =
            serde_json::from_reader(File::open(&manifest_path).unwrap()).unwrap();
        manifest["artifacts"][AO_ERI_ARTIFACT]["attributes"]["computation_version"] =
            serde_json::json!(AO_ERI_COMPUTATION_VERSION + 1);
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();

        let entries = cache.entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].verified);
        assert!(cache.load(&molecule, &basis, threshold()).is_none());
    }

    #[test]
    fn overflowing_manifest_basis_functions_are_invalid_and_not_loaded() {
        let temporary = tempfile::tempdir().unwrap();
        let cache = EriCache::new(temporary.path());
        let (molecule, basis) = input();
        cache
            .store(
                &molecule,
                &basis,
                threshold(),
                &CompactEri::Zeroed(basis.nbasis()),
            )
            .unwrap();
        let identity = ao_eri_identity(molecule.geometry(), &basis, threshold());
        let manifest_path = cache.entry_path(identity).join(MANIFEST_PATH);
        let mut manifest: serde_json::Value =
            serde_json::from_reader(File::open(&manifest_path).unwrap()).unwrap();
        manifest["artifacts"][AO_ERI_ARTIFACT]["attributes"]["basis_functions"] =
            serde_json::json!(usize::MAX);
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();

        let entries = cache.entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].verified);
        assert!(cache.load(&molecule, &basis, threshold()).is_none());
    }
    #[test]
    fn incomplete_temporary_state_is_not_a_hit() {
        let temporary = tempfile::tempdir().unwrap();
        let cache = EriCache::new(temporary.path());
        let (molecule, basis) = input();
        let identity = ao_eri_identity(molecule.geometry(), &basis, threshold());
        fs::create_dir_all(
            cache
                .entry_path(identity)
                .parent()
                .unwrap()
                .join(".rustiq-eri-interrupted"),
        )
        .unwrap();
        assert!(cache.load(&molecule, &basis, threshold()).is_none());
    }

    #[test]
    fn lists_and_removes_only_the_requested_entry() {
        let temporary = tempfile::tempdir().unwrap();
        let cache = EriCache::new(temporary.path());
        let (molecule, basis) = input();
        cache
            .store(
                &molecule,
                &basis,
                threshold(),
                &CompactEri::Zeroed(basis.nbasis()),
            )
            .unwrap();
        let fingerprint = ao_eri_identity(molecule.geometry(), &basis, threshold())
            .digest
            .to_hex();
        assert_eq!(cache.entries().unwrap().len(), 1);
        assert!(cache.remove(&fingerprint).unwrap());
        assert!(cache.entries().unwrap().is_empty());
    }

    #[test]
    fn removal_rejects_a_non_fingerprint_path() {
        let cache = EriCache::new(tempfile::tempdir().unwrap().path());
        assert_eq!(
            cache.remove("../outside").unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn removing_a_missing_invalid_entry_is_race_tolerant() {
        let entry = tempfile::tempdir().unwrap().path().join("missing");
        assert!(remove_invalid_entry(&entry).is_ok());
    }
}
