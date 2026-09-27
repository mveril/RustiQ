use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Write},
    path::PathBuf,
};

use relative_path::RelativePath;
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};

use super::{sha256_reader, ManifestError, Sha256Digest, StorageError};

#[derive(Debug)]
pub(crate) enum Storage {
    Folder(PathBuf),
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
    pub(crate) fn folder(root: impl Into<PathBuf>) -> Self {
        Self::Folder(root.into())
    }

    pub(crate) fn artifact_metadata(
        &mut self,
        path: &RelativePath,
    ) -> Result<ArtifactMetadata, StorageError> {
        match self {
            Self::Folder(root) => FolderStorage { root: root.clone() }.artifact_metadata(path),
        }
    }

    pub(crate) fn with_artifact<T, E, F>(&mut self, path: &RelativePath, read: F) -> Result<T, E>
    where
        E: From<StorageError>,
        F: FnOnce(&mut dyn Read) -> Result<T, E>,
    {
        match self {
            Self::Folder(root) => FolderStorage { root: root.clone() }.with_artifact(path, read),
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
            Self::Folder(root) => FolderStorage { root: root.clone() }.write_artifact(path, write),
        }
    }

    pub(crate) fn read_json<T: DeserializeOwned>(
        &mut self,
        path: &RelativePath,
        max_size: u64,
    ) -> Result<T, ManifestError> {
        let size = match self {
            Self::Folder(root) => FolderStorage { root: root.clone() }.artifact_size(path)?,
        };
        if size > max_size {
            return Err(ManifestError::TooLarge);
        }

        self.with_artifact(path, |reader| {
            serde_json::from_reader(reader).map_err(ManifestError::from)
        })
    }

    pub(crate) fn write_json<T: Serialize>(
        &mut self,
        path: &RelativePath,
        value: &T,
    ) -> Result<(), ManifestError> {
        self.write_artifact::<ManifestError, _>(path, |writer| {
            serde_json::to_writer_pretty(&mut *writer, value)?;
            writer.write_all(b"\n").map_err(StorageError::from)?;
            Ok(())
        })?;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<(), StorageError> {
        match self {
            Self::Folder(_) => Ok(()),
        }
    }
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
