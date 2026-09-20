use ::directories::ProjectDirs;
use rustiq_core::basis::BasisStore;
use std::{env, path::PathBuf};

const APPLICATION_NAME: &str = "RustiQ";
const DATA_HOME: &str = "RUSTIQ_DATA_HOME";
const BASIS_HOME: &str = "RUSTIQ_DATA_BASIS";
const CACHE_HOME: &str = "RUSTIQ_CACHE_HOME";

/// Resolve application directory policy before passing explicit paths to the core.
pub fn basis_store() -> BasisStore {
    BasisStore::new(&basis_path())
}

fn basis_path() -> PathBuf {
    if let Some(path) = env::var_os(BASIS_HOME) {
        return path.into();
    }

    application_data_path().join("basis_sets")
}

fn application_data_path() -> PathBuf {
    if let Some(path) = env::var_os(DATA_HOME) {
        return PathBuf::from(path).join(APPLICATION_NAME);
    }

    ProjectDirs::from("", "", APPLICATION_NAME)
        .map(|directories| directories.data_local_dir().to_path_buf())
        .unwrap_or_else(|| env::temp_dir().join(APPLICATION_NAME))
}

/// Resolve the application cache root before passing it to the scientific core.
pub fn eri_cache_path() -> PathBuf {
    if let Some(path) = env::var_os(CACHE_HOME) {
        return path.into();
    }

    ProjectDirs::from("", "", APPLICATION_NAME)
        .map(|directories| directories.cache_dir().to_path_buf())
        .unwrap_or_else(|| env::temp_dir().join(APPLICATION_NAME))
}

#[cfg(test)]
mod tests {
    // The harness-free ERI benchmark includes this module but omits test functions.
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn basis_store_uses_configured_data_home() {
        let directory = tempfile::tempdir().unwrap();
        temp_env::with_vars(
            [
                (DATA_HOME, Some(directory.path().as_os_str())),
                (BASIS_HOME, None),
            ],
            || {
                assert_eq!(
                    basis_store().path(),
                    directory.path().join("RustiQ/basis_sets")
                );
            },
        );
    }

    #[test]
    fn explicit_basis_directory_takes_precedence() {
        let directory = tempfile::tempdir().unwrap();
        let basis = directory.path().join("custom bases");
        temp_env::with_vars(
            [
                (DATA_HOME, Some(directory.path().as_os_str())),
                (BASIS_HOME, Some(basis.as_os_str())),
            ],
            || {
                assert_eq!(basis_store().path(), basis);
            },
        );
    }

    #[test]
    fn basis_store_defaults_to_project_data_directory() {
        temp_env::with_vars([(DATA_HOME, None::<&str>), (BASIS_HOME, None)], || {
            let expected = ProjectDirs::from("", "", APPLICATION_NAME)
                .map(|directories| directories.data_local_dir().to_path_buf())
                .unwrap_or_else(|| env::temp_dir().join(APPLICATION_NAME))
                .join("basis_sets");

            assert_eq!(basis_store().path(), expected);
        });
    }

    #[test]
    fn eri_cache_uses_configured_cache_home() {
        let directory = tempfile::tempdir().unwrap();
        temp_env::with_var(CACHE_HOME, Some(directory.path().as_os_str()), || {
            assert_eq!(eri_cache_path(), directory.path());
        });
    }

    #[test]
    fn eri_cache_defaults_to_project_cache_directory() {
        temp_env::with_var(CACHE_HOME, None::<&str>, || {
            let expected = ProjectDirs::from("", "", APPLICATION_NAME)
                .map(|directories| directories.cache_dir().to_path_buf())
                .unwrap_or_else(|| env::temp_dir().join(APPLICATION_NAME));
            assert_eq!(eri_cache_path(), expected);
        });
    }
}
