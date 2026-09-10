use rustiq_core::basis::BasisStore;
use std::{env, path::PathBuf};

const DATA_HOME: &str = "RUSTIQ_DATA_HOME";
const BASIS_HOME: &str = "RUSTIQ_DATA_BASIS";

/// Resolve application policy before passing an explicit path to the core.
pub fn basis_store() -> BasisStore {
    BasisStore::new(&basis_path())
}

fn basis_path() -> PathBuf {
    if let Some(path) = env::var_os(BASIS_HOME) {
        return path.into();
    }
    let data_home = env::var_os(DATA_HOME)
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::data_local_dir().unwrap_or_else(env::temp_dir));
    data_home.join("RustiQ").join("basis_sets")
}

#[cfg(feature = "online")]
pub fn auto_download_value() -> bool {
    std::env::var_os("RUSTIQ_AUTO_DOWNLOAD")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)] // This module is also included by the harness-free benchmark.
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
    fn basis_store_defaults_to_platform_data_directory() {
        temp_env::with_vars([(DATA_HOME, None::<&str>), (BASIS_HOME, None)], || {
            let expected = dirs::data_local_dir()
                .unwrap_or_else(env::temp_dir)
                .join("RustiQ")
                .join("basis_sets");
            assert_eq!(basis_store().path(), expected);
        });
    }
}
