use serde_json::Error as SerdeError;
use std::{
    fs::{self, DirEntry},
    io::{self, Read, Seek},
    path::{Component, Path, PathBuf},
};
use tempfile::NamedTempFile;
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
use crate::env::DATA_BASIS_PATH;
#[cfg(feature = "online")]
use crate::env::USER_AGENT;

#[cfg(feature = "online")]
const BASE_URL: &str = "https://www.basissetexchange.org/";

/// Struct representing a storage for basis set files.
/// This structure provides functionalities to manage, retrieve, download, and remove basis set files.
pub struct BasisStore {
    path: Box<Path>,
    #[cfg(feature = "online")]
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
    fn get_path(&self, name: &str) -> io::Result<PathBuf> {
        let mut components = Path::new(name).components();
        let is_single_normal_component =
            matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
        if name.is_empty() || name.contains(['/', '\\']) || !is_single_normal_component {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid basis set name '{name}'"),
            ));
        }

        let id = Self::basis_id(name);
        Ok(self.path.join(format!("{id}.json")))
    }

    /// BSE identifier used for cache filenames and API requests.
    fn basis_id(name: &str) -> String {
        name.to_lowercase().replace('*', "_st_")
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
        match fs::File::open(self.get_path(name)?) {
            Ok(file) => Ok(Some(BasisFile::from_reader(file)?)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
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
        let destination = self.get_path(name)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source.get_path(name)?, destination)?;
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
            .user_agent(USER_AGENT)
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
        let client = ClientBuilder::new().user_agent(USER_AGENT).build()?;
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
    /// or a [`DownloadSaveError::Save`] if the file cannot be saved.
    #[cfg(feature = "online")]
    fn basis_url(&self, name: &str) -> io::Result<Url> {
        self.get_path(name)?;
        let mut url = self.url.clone();
        url.path_segments_mut()
            .map_err(|()| io::Error::new(io::ErrorKind::InvalidInput, "invalid BSE base URL"))?
            .extend(["api", "basis", &Self::basis_id(name), "format", "json"]);
        Ok(url)
    }

    #[cfg(feature = "online")]
    pub async fn download(
        &self,
        name: &str,
        progress_callback: &mut impl FnMut(u64, Option<u64>),
    ) -> Result<(), DownloadSaveError> {
        let url = self.basis_url(name).map_err(SaveError::from)?;
        // Start downloading the file
        let client = ClientBuilder::new().user_agent(USER_AGENT).build()?;
        let mut response = client.get(url).send().await?.error_for_status()?;
        let total_size = response.content_length();
        let path = self.get_path(name).map_err(SaveError::from)?;
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid file path"))
            .map_err(SaveError::from)?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(SaveError::from)?;
        let temp_file = NamedTempFile::new_in(parent).map_err(SaveError::from)?;
        let (file, temp_path) = temp_file.into_parts();
        let mut file = tokio::fs::File::from_std(file);
        let mut downloaded: u64 = 0;
        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk).await.map_err(SaveError::from)?;
            downloaded += chunk.len() as u64;

            // Update the progress
            progress_callback(downloaded, total_size);
        }
        file.flush().await.map_err(SaveError::from)?;
        file.sync_all().await.map_err(SaveError::from)?;
        let file = file.into_std().await;
        NamedTempFile::from_parts(file, temp_path)
            .persist(path)
            .map_err(SaveError::from)?;
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
    /// or a [`DownloadSaveError::Save`] if the file cannot be saved.
    #[cfg(feature = "online")]
    #[allow(dead_code)]
    pub fn download_sync(&self, name: &str) -> Result<(), DownloadSaveError> {
        let url = self.basis_url(name).map_err(SaveError::from)?;
        // Start downloading the file
        let client = BlockingClientBuilder::new()
            .user_agent(USER_AGENT)
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
    /// This function returns an [`ImportError::Serde`] if the file cannot be parsed as a
    /// [`BasisFile`], or an [`ImportError::Save`] if it cannot be saved.
    pub fn import<R: Read + Seek>(&self, mut data: R) -> Result<String, ImportError> {
        let basis = BasisFile::from_reader(&mut data)?;
        self.import_as_raw(&basis.name, data)?;
        Ok(basis.name)
    }

    /// Imports a basis set file under the given store name after validating its content.
    #[allow(dead_code)]
    pub fn import_as<R: Read + Seek>(&self, name: &str, mut data: R) -> Result<(), ImportError> {
        BasisFile::from_reader(&mut data)?;
        self.import_as_raw(name, data)?;
        Ok(())
    }

    fn import_as_raw<R: Read + Seek>(&self, name: &str, mut data: R) -> Result<(), SaveError> {
        data.seek(io::SeekFrom::Start(0))?;
        self.save(name, &mut data)?;
        Ok(())
    }

    fn save<R: Read>(&self, name: &str, data: &mut R) -> Result<(), SaveError> {
        let path = self.get_path(name)?;
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid file path"))?;
        fs::create_dir_all(parent)?;

        let mut temp_file = NamedTempFile::new_in(parent)?;
        io::copy(data, &mut temp_file)?;
        temp_file.as_file().sync_all()?;
        temp_file.persist(path)?;
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
            match fs::remove_file(self.get_path(name.as_ref())?) {
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
        Self::new(&*DATA_BASIS_PATH)
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

/// Error type for importing a basis file into the store.
#[derive(Error, Debug)]
pub enum ImportError {
    /// The imported JSON could not be deserialized.
    #[error("Serialization error: {0}")]
    Serde(#[from] SerdeError),

    /// The validated basis file could not be saved.
    #[error(transparent)]
    Save(#[from] SaveError),
}

/// Error type for atomic basis file writes.
#[derive(Error, Debug)]
pub enum SaveError {
    /// I/O error occurred while writing the temporary file.
    #[error("I/O error while saving: {0}")]
    Io(#[from] io::Error),

    /// The completed temporary file could not replace the destination.
    #[error("failed to persist temporary file: {0}")]
    Persist(#[from] tempfile::PersistError),
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
    /// Error occurred while saving the downloaded file.
    #[error(transparent)]
    Save(#[from] SaveError),

    /// HTTP error occurred during the download.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

/// Custom error type for downloading and saving basis set files in `BasisStore`.
#[cfg(feature = "online")]
#[derive(Error, Debug)]
pub enum DownloadParseSaveError {
    /// I/O error occurred while reading a saved basis file.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Error occurred while saving the downloaded file.
    #[error(transparent)]
    Save(#[from] SaveError),

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
            DownloadSaveError::Save(e) => DownloadParseSaveError::Save(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, path::PathBuf};

    fn starred_basis() -> Vec<u8> {
        // Only the metadata is changed: these tests exercise storage, not integrals.
        let mut json: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../tests/data/sto-3g.json")).unwrap();
        json["name"] = "6-31G**".into();
        serde_json::to_vec(&json).unwrap()
    }

    #[test]
    fn test_import_get_and_remove_use_normalized_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = BasisStore::new(&dir.path());
        assert_eq!(
            store.import(io::Cursor::new(starred_basis())).unwrap(),
            "6-31G**"
        );
        assert!(dir.path().join("6-31g_st__st_.json").exists());
        for name in ["6-31G**", "6-31g**", "6-31g_st__st_"] {
            assert_eq!(store.get(name).unwrap().unwrap().name, "6-31G**");
        }
        store.remove(["6-31G**"]).unwrap();
        assert_eq!(store.list().unwrap().count(), 0);
    }

    #[cfg(feature = "online")]
    #[test]
    fn test_downloads_and_run_cache_lookup_share_id() {
        use std::io::{BufRead, Write};
        for asynchronous in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let mut store = BasisStore::new(&dir.path());
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            store.url = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = io::BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                assert!(line.starts_with("GET /api/basis/6-31g_st__st_/format/json "));
                loop {
                    line.clear();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                }
                let body = starred_basis();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            });
            if asynchronous {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(store.download("6-31G**", &mut |_, _| {}))
                    .unwrap();
            } else {
                assert_eq!(store.get_or_download("6-31G**").unwrap().name, "6-31G**");
            }
            server.join().unwrap();
            assert!(dir.path().join("6-31g_st__st_.json").exists());
            // The server is gone: online run resolution must use the cached file.
            assert_eq!(store.get_or_download("6-31g**").unwrap().name, "6-31G**");
            assert_eq!(store.list().unwrap().count(), 1);
        }
    }

    struct FailingReader(bool);

    impl Read for FailingReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if self.0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "simulated read failure",
                ));
            }

            self.0 = true;
            let partial_data = b"partial";
            buffer[..partial_data.len()].copy_from_slice(partial_data);
            Ok(partial_data.len())
        }
    }

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

    #[test]
    fn test_rejects_path_like_basis_names() {
        let store = BasisStore::new(&env::temp_dir().join("rustiq-basis-store-validation"));

        for name in ["", ".", "..", "../escape", "..\\escape", "C:\\escape"] {
            let error = store.get(name).unwrap_err();
            assert!(matches!(
                error,
                FileError::Io(ref error) if error.kind() == io::ErrorKind::InvalidInput
            ));
            assert_eq!(
                store.remove([name]).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
    }

    #[test]
    fn test_save_keeps_existing_file_when_reading_fails() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = BasisStore::new(&temp_dir.path());
        let destination = temp_dir.path().join("test.json");
        fs::write(&destination, b"existing data").unwrap();

        let error = store.save("test", &mut FailingReader(false)).unwrap_err();

        assert!(matches!(
            error,
            SaveError::Io(ref error) if error.kind() == io::ErrorKind::UnexpectedEof
        ));
        assert_eq!(fs::read(&destination).unwrap(), b"existing data");
        assert_eq!(fs::read_dir(temp_dir.path()).unwrap().count(), 1);
    }
}
