use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum StorageError {
    #[error("invalid portable archive: {0}")]
    Archive(String),
    #[error("storage I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("unsafe storage path: {0}")]
    InvalidPath(String),
    #[error("storage entry has an unexpected type: {0}")]
    UnexpectedEntryType(String),
}

/// Portable persistence failures, independent of the physical container library.
#[derive(Debug, Error)]
pub enum PortableError {
    #[error("portable artifact I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("the destination already exists")]
    AlreadyExists,
    #[error("invalid portable artifact: {0}")]
    InvalidArchive(String),
    #[error("unsupported portable calculation format or version")]
    UnsupportedVersion,
    #[error("invalid resolved calculation: {0}")]
    InvalidCalculation(String),
    #[error(transparent)]
    Artifact(#[from] ArtifactError),
}

impl From<StorageError> for PortableError {
    fn from(error: StorageError) -> Self {
        match error {
            StorageError::Io(error) => Self::Io(error),
            other => Self::InvalidArchive(other.to_string()),
        }
    }
}

impl From<PersistenceReadError> for PortableError {
    fn from(error: PersistenceReadError) -> Self {
        match error {
            PersistenceReadError::Artifact(error) => Self::Artifact(error),
            other => Self::InvalidArchive(other.to_string()),
        }
    }
}

impl From<PersistenceWriteError> for PortableError {
    fn from(error: PersistenceWriteError) -> Self {
        match error {
            PersistenceWriteError::Artifact(error) => Self::Artifact(error),
            PersistenceWriteError::Storage(error) => error.into(),
            other => Self::InvalidArchive(other.to_string()),
        }
    }
}

#[derive(Debug, Error)]
pub enum NpyError {
    #[error("could not read NPY data: {0}")]
    Read(#[source] io::Error),
    #[error("could not write NPY data: {0}")]
    Write(#[source] io::Error),
    #[error("AO ERI NPY must be one-dimensional, found shape {0:?}")]
    InvalidEriShape(Box<[u64]>),
    #[error("matrix NPY must be two-dimensional, found shape {0:?}")]
    InvalidMatrixShape(Box<[u64]>),
    #[error("NPY has shape {actual:?}, expected {expected:?}")]
    InvalidShape {
        expected: Box<[u64]>,
        actual: Box<[u64]>,
    },
    #[error("NPY dtype is not a supported f64 representation: {0}")]
    InvalidDtype(String),
    #[error(
        "AO ERI NPY has {actual} values, expected {expected} for {basis_functions} basis functions"
    )]
    InvalidValueCount {
        basis_functions: usize,
        expected: usize,
        actual: usize,
    },
    #[error("NPY dimensions exceed supported limits")]
    DimensionOverflow,
}

#[derive(Debug, Error)]
pub(crate) enum ManifestError {
    #[error("manifest storage failed: {0}")]
    Storage(#[from] StorageError),
    #[error("could not decode or encode persistence manifest: {0}")]
    Json(#[from] serde_json::Error),
    #[error("manifest is larger than the supported limit")]
    TooLarge,
    #[error("unsupported persistence format or version")]
    UnsupportedFormat,
    #[error("artifact path conflicts with another artifact: {0}")]
    ConflictingArtifactPath(String),
}

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("AO ERI artifact is missing")]
    Missing,
    #[error("unsupported artifact representation: {0}")]
    UnsupportedRepresentation(String),
    #[error("invalid artifact metadata: {0}")]
    InvalidMetadata(String),
    #[error("artifact integrity check failed: {0}")]
    IntegrityMismatch(String),
    #[error("invalid artifact path: {0}")]
    InvalidPath(String),
    #[error(
        "AO ERI payload has {actual} values, expected {expected} for {basis_functions} basis functions"
    )]
    InvalidValueCount {
        basis_functions: usize,
        expected: usize,
        actual: usize,
    },
    #[error("artifact access failed: {0}")]
    AccessFailed(String),
    #[error("artifact NPY data is invalid: {0}")]
    Npy(#[from] NpyError),
}

#[derive(Debug, Error)]
pub(crate) enum PersistenceReadError {
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error(transparent)]
    Artifact(#[from] ArtifactError),
}

#[derive(Debug, Error)]
pub(crate) enum PersistenceWriteError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error(transparent)]
    Artifact(#[from] ArtifactError),
    #[error(transparent)]
    Npy(#[from] NpyError),
}

impl From<StorageError> for ArtifactError {
    fn from(error: StorageError) -> Self {
        match error {
            StorageError::InvalidPath(path) => Self::InvalidPath(path),
            other => Self::AccessFailed(other.to_string()),
        }
    }
}
