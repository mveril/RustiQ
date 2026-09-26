//! Scientific persistence primitives and storage infrastructure.
//!
//! This module defines the stable logical persistence representation and storage
//! components such as the directory-backed deterministic artifact cache. Cache
//! location policy remains outside `rustiq-core`.

mod cache_names;
mod checksum;
mod data;
mod eri_cache;
mod identity;
mod manifest;
mod npy;

pub use crate::eri::CompactEri;
pub use checksum::{sha256, sha256_reader, verify_sha256, Sha256Digest, Sha256DigestParseError};
pub use data::{AoEriArtifact, Artifact, RustiQData};
pub use eri_cache::{EriCache, EriCacheEntry};
#[allow(unused_imports)]
pub(crate) use identity::{ao_eri_identity, ScientificIdentity, AO_ERI_COMPUTATION_VERSION};
pub use manifest::{
    AoEriAttributes, ArtifactAttributes, ArtifactManifest, Manifest, Producer,
    ScientificIdentityManifest,
};
pub use npy::NpyConvert;

#[allow(unused_imports)]
pub(crate) use npy::{
    read_compact_eri, read_dmatrix, validate_compact_eri_header, write_compact_eri,
};

pub const FORMAT_NAME: &str = "rustiq-persistence";
pub const FORMAT_VERSION: u32 = 1;
pub const MANIFEST_PATH: &str = "manifest.json";
pub const AO_ERI_PATH: &str = "arrays/integrals/ao-eri.npy";
pub const COMPACT_ERI_REPRESENTATION: &str = "rustiq-compact-eri-v1";
pub const SCIENTIFIC_IDENTITY_VERSION: u32 = 1;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("persistence I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not read or write persistence manifest: {0}")]
    Manifest(#[from] serde_json::Error),
    #[error("invalid persistence manifest: {0}")]
    InvalidManifest(String),
    #[error("invalid persistence artifact: {0}")]
    InvalidArtifact(String),
    #[error("AO ERI artifact is missing")]
    MissingEri,
    #[error("could not read NPY data: {0}")]
    NpyRead(#[source] std::io::Error),
    #[error("could not write NPY data: {0}")]
    NpyWrite(#[source] std::io::Error),
    #[error("AO ERI NPY must be one-dimensional, found shape {0:?}")]
    InvalidEriShape(Vec<u64>),
    #[error("matrix NPY must be two-dimensional, found shape {0:?}")]
    InvalidMatrixShape(Vec<u64>),
    #[error("NPY has shape {actual:?}, expected {expected:?}")]
    InvalidNpyShape {
        expected: Vec<u64>,
        actual: Vec<u64>,
    },
    #[error("AO ERI payload has {actual} values, expected {expected} for {basis_functions} basis functions")]
    InvalidValueCount {
        basis_functions: usize,
        expected: usize,
        actual: usize,
    },
    #[error("NPY dtype is not a supported f64 representation: {0}")]
    InvalidDtype(String),
}
