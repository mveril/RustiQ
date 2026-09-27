//! Scientific persistence primitives and storage infrastructure.
//!
//! This module defines the stable logical persistence representation and storage
//! components such as the directory-backed deterministic artifact cache. Cache
//! location policy remains outside `rustiq-core`.

mod cache_names;
mod checksum;
mod data;
mod eri_cache;
mod error;
mod identity;
mod manifest;
mod npy;
mod storage;

pub use crate::eri::CompactEri;
pub use checksum::{sha256, sha256_reader, verify_sha256, Sha256Digest, Sha256DigestParseError};
pub use data::{AoEriArtifact, Artifact, RustiQData};
pub use eri_cache::{EriCache, EriCacheEntry};
pub use error::{ArtifactError, NpyError};
pub(crate) use error::{ManifestError, PersistenceReadError, PersistenceWriteError, StorageError};
#[allow(unused_imports)]
pub(crate) use identity::{ao_eri_identity, ScientificIdentity, AO_ERI_COMPUTATION_VERSION};
pub(crate) use manifest::{
    AoEriAttributes, ArtifactAttributes, ArtifactManifest, Manifest, ManifestKind, Producer,
    ScientificIdentityManifest,
};
pub(crate) use storage::Storage;

#[allow(unused_imports)]
pub(crate) use npy::{
    read_compact_eri, read_dmatrix, validate_compact_eri_header, write_compact_eri,
};
pub(crate) use storage::validate_path as validate_storage_path;

pub(crate) const FORMAT_NAME: &str = "rustiq-persistence";
pub(crate) const FORMAT_VERSION: u32 = 1;
pub(crate) const MANIFEST_PATH: &str = "manifest.json";
pub(crate) const AO_ERI_PATH: &str = "arrays/integrals/ao-eri.npy";
pub(crate) const COMPACT_ERI_REPRESENTATION: &str = "rustiq-compact-eri-v1";
pub(crate) const SCIENTIFIC_IDENTITY_VERSION: u32 = 1;
