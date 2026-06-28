use serde_json::Error as SerdeError;
use std::{
    env,
    fs::{self, DirEntry, File},
    io::{self, Read, Seek},
    path::{Path, PathBuf},
};
use thiserror::Error;

#[cfg(feature = "online")]
use reqwest::{blocking::ClientBuilder as BlockingClientBuilder, ClientBuilder, Url};
#[cfg(feature = "online")]
use std::{collections::HashMap, str::FromStr};
#[cfg(feature = "online")]
use tokio::io::AsyncWriteExt;

use super::basis_file::BasisFile;
#[cfg(feature = "online")]
use super::metadata::BasisSetDetail;

#[cfg(feature = "online")]
const BASE_URL: &str = "https://www.basissetexchange.org/";

/// Struct representing a storage for basis set files.
/// This structure provides functionalities to manage, retrieve, download, and remove basis set files.
pub struct BasisStore {
    path: Box<Path>,
    #[cfg(feature = "online")]
    url: Url,
}

#[cfg(feature = "online")]
fn user_agent() -> String {
    format!(
        "{}/{} ({}; {}; +{})",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        env!("CARGO_PKG_REPOSITORY"),
    )
}

impl BasisStore {
    /// Creates a new `BasisStore` instance.
    ///
    /// # Arguments
    /// * `path` - A reference to a path where the basis files are stored.
    pub fn new(path: &impl AsRef<Path>) -> BasisStore {
        BasisStore {
            path: path.as_ref().to_owned().into_boxed_path(),
            #[cfg(feature = "online")]
            url: Url::from_str(BASE_URL).unwrap(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Constructs the full path for a given basis file name.
    ///
    /// # Arguments
    /// * `name` - The name of the basis file (without extension).
    fn get_path(&self, name: &str) -> PathBuf {
        self.path.join(format!("{name}.json"))
    }

    /// Retrieves a `BasisFile` by its name from the store.
    ///
    /// # Arguments
    /// * `name` - The name of the basis file (without extension).
    ///
    /// # Errors
    /// Returns a [`FileError::Io`] if the file cannot be opened, or
    /// [`FileError::Serde`] if it cannot be deserialized from JSON.
    pub fn get(&self, name: &str) -> Result<Option<BasisFile>, FileError> {
        let basis_path = self.get_path(name);
        if !basis_path.exists() {
            return Ok(None);
        }
        let file = fs::File::open(&basis_path)?;
        let basis_file = serde_json::from_reader(file)?;
        Ok(Some(basis_file))
    }

    /// Retrieves a `BasisFile` by its name from the store.
    ///
    /// # Arguments
    /// * `name` - The name of the basis file (without extension).
    ///
    /// # Errors
    /// Returns a [`FileError::Io`] if the file cannot be opened, or
    /// [`FileError::Serde`] if it cannot be deserialized from JSON.
    #[cfg(feature = "online")]
    pub fn get_or_download(&self, name: &str) -> Result<BasisFile, DownloadParseSaveError> {
        if let Some(data) = self.get(name)? {
            return Ok(data);
        }
        self.download_sync(name)?;
        self.get(name)?
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("basis file '{name}' was not created after download"),
                )
            })
            .map_err(Into::into)
    }

    /// Copies a basis file from another store into this store.
    #[cfg(any(test, feature = "bench-support"))]
    #[allow(dead_code)]
    pub fn copy_from(&self, source: &BasisStore, name: &str) -> io::Result<()> {
        let destination = self.get_path(name);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source.get_path(name), destination)?;
        Ok(())
    }

    /// Returns the repository fixture basis store used by tests and benches.
    #[cfg(any(test, feature = "bench-support"))]
    #[allow(dead_code)]
    pub fn repository_fixtures() -> BasisStore {
        BasisStore::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data"))
    }

    /// Lists all JSON files in the basis store directory.
    ///
    /// # Errors
    /// Returns an [`io::Result`] if the directory cannot be read.
    pub fn list(&self) -> io::Result<impl Iterator<Item = io::Result<DirEntry>>> {
        let read_dir = if !self.path.exists() {
            None
        } else {
            Some(self.path.read_dir()?)
        }
        .into_iter()
        .flatten();

        let result = read_dir.filter_map(|entry_result| match entry_result {
            Ok(entry) => {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file()
                        && entry.path().extension().and_then(|ext| ext.to_str()) == Some("json")
                    {
                        Some(Ok(entry))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Err(err) => Some(Err(err)),
        });

        Ok(result)
    }

    /// Lists all basis set metadata available online (synchronous).
    ///
    /// # Errors
    /// Returns a [`DownloadParseError::Http`] if the HTTP request fails,
    /// or [`DownloadParseError::Serde`] if the JSON response cannot be parsed.
    #[cfg(feature = "online")]
    pub fn list_online_sync(&self) -> Result<HashMap<String, BasisSetDetail>, DownloadParseError> {
        let url = format!("{}{}", self.url, "api/metadata");
        let client = BlockingClientBuilder::new()
            .user_agent(user_agent())
            .build()?;
        let basis_sets: HashMap<String, BasisSetDetail> =
            client.get(url).send()?.error_for_status()?.json()?;
        Ok(basis_sets)
    }

    /// Lists all basis set metadata available online (asynchronous).
    ///
    /// # Errors
    /// Returns a [`DownloadParseError::Http`] if the HTTP request fails,
    /// or [`DownloadParseError::Serde`] if the JSON response cannot be parsed.
    #[cfg(feature = "online")]
    #[allow(dead_code)]
    pub async fn list_online(&self) -> Result<HashMap<String, BasisSetDetail>, DownloadParseError> {
        let url = format!("{}{}", self.url, "api/metadata");
        let client = ClientBuilder::new().user_agent(user_agent()).build()?;
        let basis_sets = client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(basis_sets)
    }

    /// Downloads a basis set file asynchronously from a remote URL and saves it locally.
    /// Reports download progress through a `progress_callback` function.
    ///
    /// # Arguments
    /// * `name` - The name of the basis set to download.
    /// * `progress_callback` - A mutable reference to a function that receives progress updates (bytes downloaded, optional total size).
    ///
    /// # Errors
    /// This function returns a [`DownloadSaveError::Http`] if the HTTP request fails,
    /// or a [`DownloadSaveError::Io`] if there is an issue with file I/O.
    #[cfg(feature = "online")]
    pub async fn download(
        &self,
        name: &str,
        progress_callback: &mut impl FnMut(u64, Option<u64>),
    ) -> Result<(), DownloadSaveError> {
        let url = format!("{}api/basis/{}/format/json", self.url, name);
        // Start downloading the file
        let client = ClientBuilder::new().user_agent(user_agent()).build()?;
        let mut response = client.get(url).send().await?.error_for_status()?;
        let total_size = response.content_length();
        let path = self.get_path(name);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut file = tokio::fs::File::create(self.get_path(name)).await?;
        let mut downloaded: u64 = 0;
        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            // Update the progress
            progress_callback(downloaded, total_size);
        }
        file.flush().await?;
        Ok(())
    }

    /// Downloads a basis set file synchronously from a remote URL and saves it locally.
    ///
    /// # Arguments
    /// * `name` - The name of the basis set to download.
    /// * `progress_callback` - A mutable reference to a function that receives progress updates (bytes downloaded, optional total size).
    ///
    /// # Errors
    /// This function returns a [`DownloadSaveError::Http`] if the HTTP request fails,
    /// or a [`DownloadSaveError::Io`] if there is an issue with file I/O.
    #[cfg(feature = "online")]
    #[allow(dead_code)]
    pub fn download_sync(&self, name: &str) -> Result<(), DownloadSaveError> {
        let url = format!("{}api/basis/{}/format/json", self.url, name);
        // Start downloading the file
        let client = BlockingClientBuilder::new()
            .user_agent(user_agent())
            .build()?;
        let mut response = client.get(url).send()?.error_for_status()?;
        self.save(name, &mut response)?;
        Ok(())
    }

    /// Import a basis set file synchronously from a reader and saves it locally.
    ///
    /// # Arguments
    /// * data - The content of the basis set file to save.
    ///
    /// # Errors
    /// This function returns a [`FileError::Serde`] If the file cannot be parsed as a [`BasisFile`]
    /// or a [`FileError::Io`] if there is an issue with file I/O.
    pub fn import<R: Read + Seek>(&self, mut data: R) -> Result<String, FileError> {
        let basis = BasisFile::from_reader(&mut data)?;
        self.import_as_raw(&basis.name, data)?;
        Ok(basis.name)
    }

    /// Imports a basis set file under the given store name after validating its content.
    #[allow(dead_code)]
    pub fn import_as<R: Read + Seek>(&self, name: &str, mut data: R) -> Result<(), FileError> {
        BasisFile::from_reader(&mut data)?;
        self.import_as_raw(name, data)?;
        Ok(())
    }

    fn import_as_raw<R: Read + Seek>(&self, name: &str, mut data: R) -> io::Result<()> {
        data.seek(io::SeekFrom::Start(0))?;
        self.save(name, &mut data)?;
        Ok(())
    }

    fn save<R: Read>(&self, name: &str, mut data: &mut R) -> Result<(), io::Error> {
        let path = self.get_path(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::create(path)?;
        io::copy(&mut data, &mut file)?;
        Ok(())
    }

    /// Removes specific basis files from the store.
    ///
    /// This function accepts any type that can be converted into an iterator of strings (such as a vector of strings or an array of string slices).
    ///
    /// # Arguments
    /// * `names` - An iterator over the names of the basis files to remove (without extensions).
    ///
    /// Missing files are ignored so repeated removals are idempotent.
    ///
    /// # Errors
    /// This function returns an [`io::Result<()>`]. If any file cannot be removed for a reason other than not existing, the function will return an [`IO::Error`].
    /// It stops at the first error encountered and doesn't attempt to remove further files.
    ///
    /// # Examples
    /// ```rust
    /// # use RustiQ::basis::BasisStore;
    /// # let store = BasisStore::new(&std::env::temp_dir().join("rustiq-doc-basis-store-remove"));
    /// let names = vec!["basis1", "basis2", "basis3"];
    /// store.remove(names).expect("Failed to remove files");
    /// ```
    pub fn remove<I>(&self, names: I) -> io::Result<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        for name in names {
            let path = self.get_path(name.as_ref());
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    /// Removes all files and directories within the basis store.
    ///
    /// # Errors
    /// Returns a [`io::Error`] if the directory cannot be removed.
    pub fn remove_all(&self) -> io::Result<()> {
        if self.path.exists() {
            fs::remove_dir_all(&self.path)?;
        }
        Ok(())
    }
}

impl Default for BasisStore {
    fn default() -> Self {
        let app_local_dir = env::var_os("RUSTIQ_DATA_HOME")
            .map(PathBuf::from)
            .or_else(dirs::data_local_dir)
            .unwrap_or_else(env::temp_dir)
            .join(env!("CARGO_PKG_NAME"));
        let basis_download_path = app_local_dir.join("basis_sets");
        Self::new(&basis_download_path)
    }
}

/// Custom error type for file-related operations in `BasisStore`.
#[derive(Error, Debug)]
pub enum FileError {
    /// I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Serde JSON deserialization error.
    #[error("Serialization error: {0}")]
    Serde(#[from] SerdeError),
}

#[cfg(feature = "online")]
impl From<FileError> for DownloadParseSaveError {
    fn from(value: FileError) -> Self {
        match value {
            FileError::Io(error) => Self::Io(error),
            FileError::Serde(error) => Self::Serde(error),
        }
    }
}

/// Custom error type for errors occurring during the online listing of basis sets.
#[cfg(feature = "online")]
#[derive(Error, Debug)]
pub enum DownloadParseError {
    /// HTTP error occurred during the download.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Serde JSON deserialization error.
    #[error("Serialization error: {0}")]
    Serde(#[from] SerdeError),
}

/// Custom error type for downloading and saving basis set files in `BasisStore`.
#[cfg(feature = "online")]
#[derive(Error, Debug)]
pub enum DownloadSaveError {
    /// I/O error occurred while writing the downloaded file.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// HTTP error occurred during the download.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

/// Custom error type for downloading and saving basis set files in `BasisStore`.
#[cfg(feature = "online")]
#[derive(Error, Debug)]
pub enum DownloadParseSaveError {
    /// I/O error occurred while writing the downloaded file.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Serde JSON deserialization error.
    #[error("Serialization error: {0}")]
    Serde(#[from] SerdeError),

    /// HTTP error occurred during the download.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}
#[cfg(feature = "online")]
impl From<DownloadParseError> for DownloadParseSaveError {
    fn from(err: DownloadParseError) -> Self {
        match err {
            DownloadParseError::Http(e) => DownloadParseSaveError::Http(e),
            DownloadParseError::Serde(e) => DownloadParseSaveError::Serde(e),
        }
    }
}

#[cfg(feature = "online")]
impl From<DownloadSaveError> for DownloadParseSaveError {
    fn from(value: DownloadSaveError) -> Self {
        match value {
            DownloadSaveError::Http(e) => DownloadParseSaveError::Http(e),
            DownloadSaveError::Io(e) => DownloadParseSaveError::Io(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, path::PathBuf};

    #[test]
    fn test_default_uses_rustiq_data_home() {
        temp_env::with_var("RUSTIQ_DATA_HOME", Some("/tmp/rustiq-data-home"), || {
            let store = BasisStore::default();
            let expected = PathBuf::from("/tmp/rustiq-data-home")
                .join(env!("CARGO_PKG_NAME"))
                .join("basis_sets");
            assert_eq!(store.path(), expected);
        });
    }

    #[test]
    fn test_get_returns_none_for_missing_basis_file() {
        let temp_dir = env::temp_dir().join("rustiq-basis-store-missing");
        let store = BasisStore::new(&temp_dir);

        let basis = store.get("missing").unwrap();

        assert!(basis.is_none());
    }
}
