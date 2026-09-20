use std::{
    fs::{self, File},
    io::{self, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use tempfile::Builder;

use crate::{
    basis::Basis, config::validated::PositiveFiniteF64, eri::CompactEri,
    molecules::molecule::Molecule,
};

use super::{
    ao_eri_identity, read_compact_eri, sha256_reader, ArtifactManifest, Manifest, Producer,
    ScientificIdentity, ScientificIdentityManifest, AO_ERI_PATH, COMPACT_ERI_REPRESENTATION,
    FORMAT_NAME, FORMAT_VERSION, MANIFEST_PATH,
};

const CACHE_KIND: &str = "integral-cache";
const AO_ERI_ARTIFACT: &str = "ao_eri";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

/// A directory-backed AO ERI cache entry available for management.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EriCacheEntry {
    pub fingerprint: String,
    pub payload_size: Option<u64>,
    pub valid_manifest: bool,
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
        let parent = self.root.join("eri");
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
            let manifest_path = entry.path().join(MANIFEST_PATH);
            let manifest = fs::metadata(&manifest_path)
                .ok()
                .filter(|metadata| metadata.len() <= MAX_MANIFEST_BYTES)
                .and_then(|_| File::open(manifest_path).ok())
                .and_then(|file| serde_json::from_reader::<_, Manifest>(BufReader::new(file)).ok());
            let artifact = manifest
                .as_ref()
                .and_then(|manifest| manifest.artifacts.get(AO_ERI_ARTIFACT));
            let valid_manifest = manifest.as_ref().is_some_and(|manifest| {
                manifest.format == FORMAT_NAME
                    && manifest.format_version == FORMAT_VERSION
                    && manifest.kind == CACHE_KIND
                    && manifest.scientific_identity.digest.to_hex() == fingerprint
                    && artifact.is_some_and(|artifact| {
                        artifact.path == AO_ERI_PATH
                            && artifact.representation == COMPACT_ERI_REPRESENTATION
                    })
            });
            result.push(EriCacheEntry {
                fingerprint,
                payload_size: artifact.map(|artifact| artifact.size),
                valid_manifest,
            });
        }
        result.sort_by(|left, right| left.fingerprint.cmp(&right.fingerprint));
        Ok(result)
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
        Ok(true)
    }

    /// Removes every published ERI cache entry below this cache root.
    pub fn remove_all(&self) -> io::Result<()> {
        for entry in self.entries()? {
            self.remove(&entry.fingerprint)?;
        }
        Ok(())
    }

    pub(crate) fn load(
        &self,
        molecule: &Molecule,
        basis: &Basis,
        threshold: Option<PositiveFiniteF64>,
    ) -> Option<CompactEri> {
        self.load_identity(
            ao_eri_identity(molecule.geometry(), basis, threshold),
            basis.nbasis(),
        )
    }

    pub(crate) fn store(
        &self,
        molecule: &Molecule,
        basis: &Basis,
        threshold: Option<PositiveFiniteF64>,
        eri: &CompactEri,
    ) -> io::Result<()> {
        self.store_identity(
            ao_eri_identity(molecule.geometry(), basis, threshold),
            basis.nbasis(),
            eri,
        )
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
        let manifest_path = entry.join(MANIFEST_PATH);
        if fs::metadata(&manifest_path).ok()?.len() > MAX_MANIFEST_BYTES {
            return None;
        }
        let manifest: Manifest =
            serde_json::from_reader(BufReader::new(File::open(manifest_path).ok()?)).ok()?;
        let artifact = manifest.artifacts.get(AO_ERI_ARTIFACT)?;
        if manifest.format != FORMAT_NAME
            || manifest.format_version != FORMAT_VERSION
            || manifest.kind != CACHE_KIND
            || manifest.scientific_identity.version != identity.version
            || manifest.scientific_identity.digest != identity.digest
            || artifact.path != AO_ERI_PATH
            || artifact.representation != COMPACT_ERI_REPRESENTATION
            || artifact.basis_functions != basis_functions
        {
            return None;
        }
        let payload_path = entry.join(AO_ERI_PATH);
        let metadata = fs::metadata(&payload_path).ok()?;
        let digest = sha256_reader(BufReader::new(File::open(&payload_path).ok()?)).ok()?;
        if metadata.len() != artifact.size || digest != artifact.digest {
            return None;
        }
        read_compact_eri(
            BufReader::new(File::open(payload_path).ok()?),
            basis_functions,
        )
        .ok()
    }

    fn store_identity(
        &self,
        identity: ScientificIdentity,
        basis_functions: usize,
        eri: &CompactEri,
    ) -> io::Result<()> {
        let final_entry = self.entry_path(identity);
        if final_entry.exists() {
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
        match fs::rename(&temporary_path, &final_entry) {
            Ok(()) => Ok(()),
            Err(_) if final_entry.exists() => {
                let _ = fs::remove_dir_all(temporary_path);
                Ok(())
            }
            Err(error) => {
                let _ = fs::remove_dir_all(temporary_path);
                Err(error)
            }
        }
    }
}

fn is_fingerprint(value: &str) -> bool {
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
}
