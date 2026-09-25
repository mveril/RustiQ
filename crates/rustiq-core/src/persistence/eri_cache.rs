use std::{
    fs::{self, File},
    io::{self, BufReader, BufWriter, Seek, Write},
    path::{Path, PathBuf},
};

use tempfile::Builder;

use crate::{
    basis::Basis, config::validated::PositiveFiniteF64, eri::CompactEri,
    molecules::molecule::Molecule,
};

use super::{
    ao_eri_identity, read_compact_eri, sha256_reader, validate_compact_eri_header,
    ArtifactManifest, Manifest, Producer, ScientificIdentity, ScientificIdentityManifest,
    AO_ERI_PATH, COMPACT_ERI_REPRESENTATION, FORMAT_NAME, FORMAT_VERSION, MANIFEST_PATH,
    SCIENTIFIC_IDENTITY_VERSION,
};

const CACHE_KIND: &str = "integral-cache";
const AO_ERI_ARTIFACT: &str = "ao_eri";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

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
                        validate_payload(&entry_path, artifact, artifact.basis_functions)
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
        let manifest = read_manifest(&entry)?;
        let artifact = manifest.artifacts.get(AO_ERI_ARTIFACT)?;
        if !manifest_is_valid(&manifest, identity) || artifact.basis_functions != basis_functions {
            return None;
        }
        read_validated_payload(&entry, artifact, basis_functions)
    }

    fn store_identity(
        &self,
        identity: ScientificIdentity,
        basis_functions: usize,
        eri: &CompactEri,
    ) -> io::Result<()> {
        let final_entry = self.entry_path(identity);
        if self.load_identity(identity, basis_functions).is_some() {
            return Ok(());
        }
        let parent = final_entry.parent().expect("ERI cache entry has a parent");
        fs::create_dir_all(parent)?;
        let temporary = Builder::new().prefix(".rustiq-eri-").tempdir_in(parent)?;
        let payload_path = temporary.path().join(AO_ERI_PATH);
        fs::create_dir_all(payload_path.parent().expect("AO ERI path has a parent"))?;
        {
            let mut writer = BufWriter::new(File::create(&payload_path)?);
            super::write_compact_eri(&mut writer, eri)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            writer.flush()?;
            writer.into_inner()?.sync_all()?;
        }
        let payload_metadata = fs::metadata(&payload_path)?;
        let payload_digest = sha256_reader(BufReader::new(File::open(&payload_path)?))?;
        let manifest = Manifest {
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
            artifacts: [(
                AO_ERI_ARTIFACT.to_owned(),
                ArtifactManifest {
                    path: AO_ERI_PATH.to_owned(),
                    size: payload_metadata.len(),
                    representation: COMPACT_ERI_REPRESENTATION.to_owned(),
                    basis_functions,
                    digest: payload_digest,
                },
            )]
            .into_iter()
            .collect(),
        };
        {
            let mut writer = BufWriter::new(File::create(temporary.path().join(MANIFEST_PATH))?);
            serde_json::to_writer_pretty(&mut writer, &manifest).map_err(io::Error::other)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
            writer.into_inner()?.sync_all()?;
        }
        let temporary_path = temporary.keep();
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
        match fs::rename(&temporary_path, &final_entry) {
            Ok(()) => Ok(()),
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
    let manifest_path = entry.join(MANIFEST_PATH);
    fs::symlink_metadata(&manifest_path)
        .ok()
        .filter(|metadata| {
            metadata.is_file()
                && !metadata.file_type().is_symlink()
                && metadata.len() <= MAX_MANIFEST_BYTES
        })?;
    serde_json::from_reader(BufReader::new(File::open(manifest_path).ok()?)).ok()
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
            })
}

fn read_validated_payload(
    entry: &Path,
    artifact: &ArtifactManifest,
    basis_functions: usize,
) -> Option<CompactEri> {
    CompactEri::checked_storage_len(basis_functions)?;
    let payload_path = entry.join(AO_ERI_PATH);
    let metadata = fs::symlink_metadata(&payload_path).ok()?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return None;
    }
    let mut file = File::open(payload_path).ok()?;
    if file.metadata().ok()?.len() != artifact.size {
        return None;
    }
    if sha256_reader(&mut file).ok()? != artifact.digest {
        return None;
    }
    file.rewind().ok()?;
    read_compact_eri(file, basis_functions).ok()
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
        manifest["artifacts"][AO_ERI_ARTIFACT]["basis_functions"] = serde_json::json!(usize::MAX);
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
