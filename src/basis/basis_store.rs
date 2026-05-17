use reqwest::{blocking::get as blocking_get, get, Url};
use serde_json::Error as SerdeError;
use std::{
    collections::HashMap,
    fs::{self, DirEntry},
    io,
    path::{Path, PathBuf},
    str::FromStr,
};
use thiserror::Error;
use tokio::io::AsyncWriteExt;

use super::metadata::BasisSetDetail;
use super::basisfile::BasisFile;
const BASE_URL: &str = "https://www.basissetexchange.org/";

/// Struct representing a storage for basis set files.
/// This structure provides functionalities to manage, retrieve, download, and remove basis set files.
pub struct BasisStore {
    path: Box<Path>,
    url: Url,
}

impl BasisStore {
    /// Creates a new `BasisStore` instance.
    ///
    /// # Arguments
    /// * `path` - A reference to a path where the basis files are stored.
    pub fn new(path: &impl AsRef<Path>) -> BasisStore {
        BasisStore {
            path: path.as_ref().to_owned().into_boxed_path(),
            url: Url::from_str(BASE_URL).unwrap(),
        }
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
    pub fn get(&self, name: &str) -> Result<BasisFile, FileError> {
        let basis_path = self.get_path(name);
        let file = fs::File::open(&basis_path)?;
        let basis_file = serde_json::from_reader(file)?;

        Ok(basis_file)
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
    pub fn list_online_sync(&self) -> Result<HashMap<String, BasisSetDetail>, DownloadParseError> {
        let url = format!("{}{}", self.url, "api/metadata");
        let basis_sets: HashMap<String, BasisSetDetail> =
            blocking_get(url)?.error_for_status()?.json()?;
        Ok(basis_sets)
    }

    /// Lists all basis set metadata available online (asynchronous).
    ///
    /// # Errors
    /// Returns a [`DownloadParseError::Http`] if the HTTP request fails,
    /// or [`DownloadParseError::Serde`] if the JSON response cannot be parsed.
    #[allow(dead_code)]
    pub async fn list_online(&self) -> Result<HashMap<String, BasisSetDetail>, DownloadParseError> {
        let url = format!("{}{}", self.url, "api/metadata");
        let basis_sets = get(url).await?.error_for_status()?.json().await?;
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
    pub async fn download(
        &self,
        name: &str,
        progress_callback: &mut impl FnMut(u64, Option<u64>),
    ) -> Result<(), DownloadSaveError> {
        let url = format!("{}api/basis/{}/format/json", self.url, name);
        // Start downloading the file
        let mut response = get(&url).await?.error_for_status()?;
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
    #[allow(dead_code)]
    pub fn download_sync(&self, name: &str) -> Result<(), DownloadSaveError> {
        let url = format!("{}api/basis/{}/format/json", self.url, name);
        // Start downloading the file
        let mut response = blocking_get(&url)?.error_for_status()?;
        let path = self.get_path(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(self.get_path(name))?;
        response.copy_to(&mut file)?;
        Ok(())
    }

    /// Removes specific basis files from the store.
    ///
    /// This function accepts any type that can be converted into an iterator of strings (such as a vector of strings or an array of string slices).
    ///
    /// # Arguments
    /// * `names` - An iterator over the names of the basis files to remove (without extensions).
    ///
    /// # Errors
    /// This function returns an [`io::Result<()>`]. If any file cannot be removed, the function will return an [`IO::Error`].
    /// It stops at the first error encountered and doesn't attempt to remove further files.
    ///
    /// # Examples
    /// ```rust
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
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Removes all files and directories within the basis store.
    ///
    /// # Errors
    /// Returns a [`io::Result`] if the directory cannot be removed.
    pub fn remove_all(&self) -> io::Result<()> {
        if self.path.exists() {
            fs::remove_dir_all(&self.path)?;
        }
        Ok(())
    }
}

impl Default for BasisStore {
    fn default() -> Self {
        let app_local_dir = dirs::data_local_dir()
            .expect("Could not find local data directory")
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

/// Custom error type for errors occurring during the online listing of basis sets.
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
#[derive(Error, Debug)]
pub enum DownloadSaveError {
    /// I/O error occurred while writing the downloaded file.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// HTTP error occurred during the download.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}
