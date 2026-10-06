use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Write},
    path::PathBuf,
};

use relative_path::RelativePath;
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};

use super::{sha256_reader, ManifestError, Sha256Digest, StorageError};

mod zip;

#[derive(Debug)]
pub(crate) enum Storage {
    Folder(PathBuf),
    Scoped {
        storage: std::sync::Arc<std::sync::Mutex<Storage>>,
        prefix: String,
    },
    ZipReader(Box<::zip::ZipArchive<File>>),
    ZipWriter(Box<::zip::ZipWriter<BufWriter<File>>>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ArtifactMetadata {
    pub(crate) size: u64,
    pub(crate) digest: Sha256Digest,
}

struct DigestWriter<W> {
    inner: W,
    hasher: Sha256,
    size: u64,
}

impl<W> DigestWriter<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            size: 0,
        }
    }

    fn finish(self) -> (W, ArtifactMetadata) {
        (
            self.inner,
            ArtifactMetadata {
                size: self.size,
                digest: self.hasher.finalize().into(),
            },
        )
    }
}

impl<W: Write> Write for DigestWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buffer)?;
        self.hasher.update(&buffer[..written]);
        let written_u64 =
            u64::try_from(written).map_err(|_| io::Error::other("artifact size exceeds u64"))?;
        self.size = self
            .size
            .checked_add(written_u64)
            .ok_or_else(|| io::Error::other("artifact size exceeds u64"))?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl Storage {
    pub(crate) fn scoped(storage: std::sync::Arc<std::sync::Mutex<Self>>, prefix: String) -> Self {
        Self::Scoped { storage, prefix }
    }

    pub(crate) fn open_zip(path: &std::path::Path) -> Result<Self, StorageError> {
        Ok(Self::ZipReader(Box::new(zip::open(path)?)))
    }

    pub(crate) fn create_zip(file: File) -> Self {
        Self::ZipWriter(Box::new(zip::writer(file)))
    }

    pub(crate) fn artifact_size(&mut self, path: &RelativePath) -> Result<u64, StorageError> {
        validate_path(path)?;
        match self {
            Self::Scoped { storage, prefix } => storage
                .lock()
                .expect("storage mutex poisoned")
                .artifact_size(&prefixed_path(prefix, path)?),
            Self::Folder(root) => FolderStorage { root: root.clone() }.artifact_size(path),
            Self::ZipReader(archive) => Ok(archive
                .by_name(path.as_str())
                .map_err(zip::zip_error)?
                .size()),
            Self::ZipWriter(_) => Err(zip::invalid("cannot read an archive writer")),
        }
    }

    pub(crate) fn folder(root: impl Into<PathBuf>) -> Self {
        Self::Folder(root.into())
    }

    pub(crate) fn artifact_metadata(
        &mut self,
        path: &RelativePath,
    ) -> Result<ArtifactMetadata, StorageError> {
        match self {
            Self::Scoped { storage, prefix } => storage
                .lock()
                .expect("storage mutex poisoned")
                .artifact_metadata(&prefixed_path(prefix, path)?),
            Self::Folder(root) => FolderStorage { root: root.clone() }.artifact_metadata(path),
            Self::ZipReader(archive) => {
                let size = archive
                    .by_name(path.as_str())
                    .map_err(zip::zip_error)?
                    .size();
                let digest = zip::with_member::<_, StorageError, _>(archive, path, |reader| {
                    Ok(sha256_reader(reader)?)
                })?;
                Ok(ArtifactMetadata { size, digest })
            }
            Self::ZipWriter(_) => Err(zip::invalid("cannot read an archive writer")),
        }
    }

    pub(crate) fn with_artifact<T, E, F>(&mut self, path: &RelativePath, read: F) -> Result<T, E>
    where
        E: From<StorageError>,
        F: FnOnce(&mut dyn Read) -> Result<T, E>,
    {
        match self {
            Self::Scoped { storage, prefix } => storage
                .lock()
                .expect("storage mutex poisoned")
                .with_artifact(&prefixed_path(prefix, path).map_err(E::from)?, read),
            Self::Folder(root) => FolderStorage { root: root.clone() }.with_artifact(path, read),
            Self::ZipReader(archive) => zip::with_member(archive, path, read),
            Self::ZipWriter(_) => Err(E::from(zip::invalid("cannot read an archive writer"))),
        }
    }

    pub(crate) fn write_artifact<E, F>(
        &mut self,
        path: &RelativePath,
        write: F,
    ) -> Result<ArtifactMetadata, E>
    where
        E: From<StorageError>,
        F: FnOnce(&mut dyn Write) -> Result<(), E>,
    {
        match self {
            Self::Scoped { storage, prefix } => storage
                .lock()
                .expect("storage mutex poisoned")
                .write_artifact(&prefixed_path(prefix, path).map_err(E::from)?, write),
            Self::Folder(root) => FolderStorage { root: root.clone() }.write_artifact(path, write),
            Self::ZipWriter(archive) => {
                validate_path(path).map_err(E::from)?;
                archive
                    .start_file(path.as_str(), zip::options(path))
                    .map_err(zip::zip_error)
                    .map_err(E::from)?;
                let mut writer = DigestWriter::new(archive.as_mut());
                write(&mut writer)?;
                let (_, metadata) = writer.finish();
                Ok(metadata)
            }
            Self::ZipReader(_) => Err(E::from(zip::invalid("cannot write an archive reader"))),
        }
    }

    pub(crate) fn read_json<T: DeserializeOwned>(
        &mut self,
        path: &RelativePath,
        max_size: u64,
    ) -> Result<T, ManifestError> {
        let size = self.artifact_size(path)?;
        if size > max_size {
            return Err(ManifestError::TooLarge);
        }

        self.with_artifact(path, |reader| {
            let mut bytes = Vec::new();
            reader
                .take(max_size + 1)
                .read_to_end(&mut bytes)
                .map_err(StorageError::from)?;
            if u64::try_from(bytes.len()).map_err(|_| ManifestError::TooLarge)? > max_size {
                return Err(ManifestError::TooLarge);
            }
            serde_json::from_slice(&bytes).map_err(ManifestError::from)
        })
    }

    pub(crate) fn write_json<T: Serialize>(
        &mut self,
        path: &RelativePath,
        value: &T,
    ) -> Result<ArtifactMetadata, ManifestError> {
        let mut bytes = serde_json::to_vec_pretty(value)?;
        bytes.push(b'\n');
        let limit = match path.file_name().unwrap_or_default() {
            super::calculation::CALCULATION_PATH => super::calculation::MAX_CALCULATION_BYTES,
            super::request::REQUEST_PATH => super::request::MAX_REQUEST_BYTES,
            _ => 1024 * 1024,
        };
        if u64::try_from(bytes.len()).map_err(|_| ManifestError::TooLarge)? > limit {
            return Err(ManifestError::TooLarge);
        }
        self.write_artifact::<ManifestError, _>(path, |writer| {
            writer.write_all(&bytes).map_err(StorageError::from)?;
            Ok(())
        })
    }

    pub(crate) fn finish(self) -> Result<(), StorageError> {
        match self {
            Self::Folder(_) | Self::ZipReader(_) | Self::Scoped { .. } => Ok(()),
            Self::ZipWriter(archive) => {
                let mut file = archive.finish().map_err(zip::zip_error)?;
                file.flush()?;
                file.get_ref().sync_all()?;
                Ok(())
            }
        }
    }
}

fn prefixed_path(
    prefix: &str,
    path: &RelativePath,
) -> Result<relative_path::RelativePathBuf, StorageError> {
    validate_path(path)?;
    let full = relative_path::RelativePathBuf::from(format!("{prefix}/{}", path.as_str()));
    validate_path(&full)?;
    Ok(full)
}

#[derive(Debug)]
struct FolderStorage {
    root: PathBuf,
}

impl FolderStorage {
    fn artifact_size(&self, path: &RelativePath) -> Result<u64, StorageError> {
        Ok(self.open_artifact(path)?.metadata()?.len())
    }

    fn artifact_metadata(&self, path: &RelativePath) -> Result<ArtifactMetadata, StorageError> {
        let file = self.open_artifact(path)?;
        let size = file.metadata()?.len();
        let digest = sha256_reader(BufReader::new(file))?;
        Ok(ArtifactMetadata { size, digest })
    }

    fn with_artifact<T, E, F>(&self, path: &RelativePath, read: F) -> Result<T, E>
    where
        E: From<StorageError>,
        F: FnOnce(&mut dyn Read) -> Result<T, E>,
    {
        let mut reader = BufReader::new(self.open_artifact(path).map_err(E::from)?);
        read(&mut reader)
    }

    fn write_artifact<E, F>(&mut self, path: &RelativePath, write: F) -> Result<ArtifactMetadata, E>
    where
        E: From<StorageError>,
        F: FnOnce(&mut dyn Write) -> Result<(), E>,
    {
        let file = self.create_artifact(path).map_err(E::from)?;
        let writer = BufWriter::new(file);
        let mut writer = DigestWriter::new(writer);
        write(&mut writer)?;
        writer
            .flush()
            .map_err(StorageError::from)
            .map_err(E::from)?;
        let (writer, metadata) = writer.finish();
        let file = writer
            .into_inner()
            .map_err(|error| StorageError::Io(error.into_error()))
            .map_err(E::from)?;
        file.sync_all()
            .map_err(StorageError::from)
            .map_err(E::from)?;
        Ok(metadata)
    }

    fn open_artifact(&self, path: &RelativePath) -> Result<File, StorageError> {
        validate_path(path)?;
        let components: Vec<_> = path.as_str().split('/').collect();
        let mut native = self.root.clone();

        for (index, component) in components.iter().enumerate() {
            native.push(component);
            let metadata = fs::symlink_metadata(&native)?;
            if metadata.file_type().is_symlink() {
                return Err(StorageError::UnexpectedEntryType(path.to_string()));
            }

            let is_last = index + 1 == components.len();
            if (!is_last && !metadata.is_dir()) || (is_last && !metadata.is_file()) {
                return Err(StorageError::UnexpectedEntryType(path.to_string()));
            }
        }

        Ok(File::open(native)?)
    }

    fn create_artifact(&self, path: &RelativePath) -> Result<File, StorageError> {
        validate_path(path)?;
        let root_metadata = fs::symlink_metadata(&self.root)?;
        if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
            return Err(StorageError::UnexpectedEntryType(
                self.root.display().to_string(),
            ));
        }

        let mut components = path.as_str().split('/').peekable();
        let mut parent = self.root.clone();
        while let Some(component) = components.next() {
            if components.peek().is_none() {
                parent.push(component);
                return Ok(OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(parent)?);
            }

            parent.push(component);
            match fs::symlink_metadata(&parent) {
                Ok(metadata) => {
                    if !metadata.is_dir() || metadata.file_type().is_symlink() {
                        return Err(StorageError::UnexpectedEntryType(path.to_string()));
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    fs::create_dir(&parent)?;
                }
                Err(error) => return Err(error.into()),
            }
        }

        Err(StorageError::InvalidPath(path.to_string()))
    }
}

pub(crate) fn validate_path(path: &RelativePath) -> Result<(), StorageError> {
    let value = path.as_str();
    if value.is_empty() || value.starts_with('/') || value.contains('\\') {
        return Err(StorageError::InvalidPath(path.to_string()));
    }

    for component in value.split('/') {
        if component.is_empty()
            || matches!(component, "." | "..")
            || component.ends_with(['.', ' '])
            || component.chars().any(|character| {
                character < '\u{20}' || matches!(character, '<' | '>' | ':' | '"' | '|' | '?' | '*')
            })
            || is_windows_reserved_name(component)
        {
            return Err(StorageError::InvalidPath(path.to_string()));
        }
    }

    Ok(())
}

fn is_windows_reserved_name(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or(component)
        .to_ascii_uppercase();

    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || (stem.len() == 4
        && matches!(&stem[..3], "COM" | "LPT")
        && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_storage_streams_artifacts_and_json() {
        let root = tempfile::tempdir().unwrap();
        let mut storage = Storage::folder(root.path());
        let artifact_path = RelativePath::new("arrays/test.bin");

        let metadata = storage
            .write_artifact::<StorageError, _>(artifact_path, |writer| {
                writer.write_all(b"payload")?;
                Ok(())
            })
            .unwrap();
        assert_eq!(metadata.size, 7);
        assert_eq!(metadata.digest, super::super::sha256(b"payload"));

        let payload = storage
            .with_artifact::<_, StorageError, _>(artifact_path, |reader| {
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes)?;
                Ok(bytes)
            })
            .unwrap();
        assert_eq!(payload, b"payload");

        let manifest_path = RelativePath::new("manifest.json");
        storage
            .write_json(manifest_path, &serde_json::json!({"version": 1}))
            .unwrap();
        let manifest: serde_json::Value = storage.read_json(manifest_path, 1024).unwrap();
        assert_eq!(manifest["version"], 1);
        storage.finish().unwrap();
    }

    #[test]
    fn rejects_non_portable_windows_paths() {
        for path in [
            "CON",
            "con.npy",
            "arrays/NUL.bin",
            "arrays/COM1.npy",
            "arrays/LPT9.npy",
            "arrays/trailing.",
            "arrays/trailing ",
            "arrays/bad?.npy",
            "arrays/bad|name.npy",
        ] {
            assert!(validate_path(RelativePath::new(path)).is_err(), "{path}");
        }
    }
}
