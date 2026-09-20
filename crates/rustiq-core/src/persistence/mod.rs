//! Storage-independent primitives for persisted scientific artifacts.
//!
//! This module defines the stable logical representation; it does not choose a
//! cache directory or container format.

mod checksum;
mod identity;
mod manifest;
mod npy;

pub use checksum::{sha256, verify_sha256, Sha256Digest, Sha256DigestParseError};
#[allow(unused_imports)]
pub(crate) use identity::{ao_eri_identity, ScientificIdentity};
pub use manifest::{ArtifactManifest, Manifest, Producer, ScientificIdentityManifest};

#[allow(unused_imports)]
pub(crate) use npy::{read_compact_eri, write_compact_eri};

pub const FORMAT_NAME: &str = "rustiq-persistence";
pub const FORMAT_VERSION: u32 = 1;
pub const MANIFEST_PATH: &str = "manifest.json";
pub const AO_ERI_PATH: &str = "arrays/integrals/ao-eri.npy";
pub const COMPACT_ERI_REPRESENTATION: &str = "rustiq-compact-eri-v1";
pub const SCIENTIFIC_IDENTITY_VERSION: u32 = 1;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("could not read NPY data: {0}")]
    NpyRead(#[source] std::io::Error),
    #[error("could not write NPY data: {0}")]
    NpyWrite(#[source] std::io::Error),
    #[error("AO ERI NPY must be one-dimensional, found shape {0:?}")]
    InvalidShape(Vec<u64>),
    #[error("AO ERI payload has {actual} values, expected {expected} for {basis_functions} basis functions")]
    InvalidValueCount {
        basis_functions: usize,
        expected: usize,
        actual: usize,
    },
    #[error("AO ERI NPY dtype is not a supported f64 representation: {0}")]
    InvalidDtype(String),
}
