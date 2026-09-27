use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Write},
    path::PathBuf,
};

use relative_path::RelativePath;
use serde::{de::DeserializeOwned, Serialize};

use super::{sha256_reader, PersistenceError, Sha256Digest};

#[derive(Debug)]
pub(crate) enum Storage {
    Folder(FolderStorage),
    // Later: Zip(ZipStorage).
}

#[derive(Debug)]
pub(crate) struct FolderStorage {
    root: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ArtifactMetadata {
    pub(crate) size: u64,
    pub(crate) digest: Sha256Digest,
}

impl Storage {
    pub(crate) fn folder(root: impl Into<PathBuf>) -> Self {
        Self::Folder(FolderStorage { root: root.into() })
    }

    pub(crate) fn artifact_metadata(
        &self,
        path: &RelativePath,
    ) -> Result<ArtifactMetadata, PersistenceError> {
        match self {
            Self::Folder(folder) => folder.artifact_metadata(path),
        }
    }

    pub(crate) fn with_artifact<T, F>(
        &self,
        path: &RelativePath,
        read: F,
    ) -> Result<T, PersistenceError>
    where
        F: FnOnce(&mut dyn Read) -> Result<T, PersistenceError>,
    {
        match self {
            Self::Folder(folder) => folder.with_artifact(path, read),
        }
    }

    pub(crate) fn write_artifact<F>(
        &mut self,
        path: &RelativePath,
        write: F,
    ) -> Result<ArtifactMetadata, PersistenceError>
    where
        F: FnOnce(&mut dyn Write) -> Result<(), PersistenceError>,
    {
        match self {
            Self::Folder(folder) => folder.write_artifact(path, write),
        }
    }

    pub(crate) fn read_json<T: DeserializeOwned>(
        &self,
        path: &RelativePath,
        max_size: u64,
    ) -> Result<T, PersistenceError> {
        let size = match self {
            Self::Folder(folder) => folder.artifact_size(path)?,
        };
        if size > max_size {
            return Err(PersistenceError::InvalidManifest(
                "manifest is larger than the supported limit".into(),
            ));
        }

        self.with_artifact(path, |reader| {
            serde_json::from_reader(reader).map_err(PersistenceError::from)
        })
    }

    pub(crate) fn write_json<T: Serialize>(
        &mut self,
        path: &RelativePath,
        value: &T,
    ) -> Result<(), PersistenceError> {
        self.write_artifact(path, |writer| {
            serde_json::to_writer_pretty(&mut *writer, value)?;
            writer.write_all(b"\n")?;
            Ok(())
        })?;
        Ok(())
    }
}

impl FolderStorage {
    fn artifact_size(&self, path: &RelativePath) -> Result<u64, PersistenceError> {
        Ok(self.open_artifact(path)?.metadata()?.len())
    }

    fn artifact_metadata(
        &self,
        path: &RelativePath,
    ) -> Result<ArtifactMetadata, PersistenceError> {
        let file = self.open_artifact(path)?;
        let size = file.metadata()?.len();
        let digest = sha256_reader(BufReader::new(file))?;
        Ok(ArtifactMetadata { size, digest })
    }

    fn with_artifact<T, F>(
        &self,
        path: &RelativePath,
        read: F,
    ) -> Result<T, PersistenceError>
    where
        F: FnOnce(&mut dyn Read) -> Result<T, PersistenceError>,
    {
        let mut reader = BufReader::new(self.open_artifact(path)?);
        read(&mut reader)
    }

    fn write_artifact<F>(
        &mut self,
        path: &RelativePath,
        write: F,
    ) -> Result<ArtifactMetadata, PersistenceError>
    where
        F: FnOnce(&mut dyn Write) -> Result<(), PersistenceError>,
    {
        let file = self.create_artifact(path)?;
        let mut writer = BufWriter::new(file);
        write(&mut writer)?;
        writer.flush()?;
        let file = writer
            .into_inner()
            .map_err(|error| PersistenceError::Io(error.into_error()))?;
        file.sync_all()?;
        drop(file);
        self.artifact_metadata(path)
    }

    fn open_artifact(&self, path: &RelativePath) -> Result<File, PersistenceError> {
        validate_path(path)?;
        let components: Vec<_> = path.as_str().split('/').collect();
        let mut native = self.root.clone();

        for (index, component) in components.iter().enumerate() {
            native.push(component);
            let metadata = fs::symlink_metadata(&native)?;
            if metadata.file_type().is_symlink() {
                return Err(PersistenceError::InvalidArtifact(format!(
                    "artifact contains a symbolic link: {path}"
                )));
            }

            let is_last = index + 1 == components.len();
            if (!is_last && !metadata.is_dir()) || (is_last && !metadata.is_file()) {
                return Err(PersistenceError::InvalidArtifact(format!(
                    "artifact path has an unexpected file type: {path}"
                )));
            }
        }

        Ok(File::open(native)?)
    }

    fn create_artifact(&self, path: &RelativePath) -> Result<File, PersistenceError> {
        validate_path(path)?;
        let root_metadata = fs::symlink_metadata(&self.root)?;
        if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
            return Err(PersistenceError::InvalidArtifact(
                "storage root is not a regular directory".into(),
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
                        return Err(PersistenceError::InvalidArtifact(format!(
                            "artifact parent is not a regular directory: {path}"
                        )));
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    fs::create_dir(&parent)?;
                }
                Err(error) => return Err(error.into()),
            }
        }

        Err(PersistenceError::InvalidArtifact(
            "artifact path is empty".into(),
        ))
    }
}

pub(crate) fn validate_path(path: &RelativePath) -> Result<(), PersistenceError> {
    let value = path.as_str();
    let invalid_component = value
        .split('/')
        .any(|component| component.is_empty() || matches!(component, "." | ".."));

    if value.is_empty()
        || value.starts_with('/')
        || value.contains('\\')
        || value.contains(':')
        || invalid_component
    {
        return Err(PersistenceError::InvalidArtifact(format!(
            "unsafe artifact path: {path}"
        )));
    }

    Ok(())
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
            .write_artifact(artifact_path, |writer| {
                writer.write_all(b"payload")?;
                Ok(())
            })
            .unwrap();
        assert_eq!(metadata.size, 7);

        let payload = storage
            .with_artifact(artifact_path, |reader| {
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
    }
}
