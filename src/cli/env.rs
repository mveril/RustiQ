use std::{
    env,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use dirs::data_local_dir;

pub const DATA_HOME: &str = "RUSTIQ_DATA_HOME";
pub const BASIS_HOME: &str = "RUSTIQ_DATA_BASIS";
pub const AUTO_DOWNLOAD: &str = "RUSTIQ_AUTO_DOWNLOAD";
pub const AUTO_DOWNLOAD_DEFAULT: bool = false;
use const_format::formatcp;

pub const USER_AGENT: &str = formatcp!(
    "{}/{} ({}; {}; +{})",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_VERSION"),
    std::env::consts::OS,
    std::env::consts::ARCH,
    env!("CARGO_PKG_REPOSITORY"),
);

pub static DATA_HOME_PATH: LazyLock<Box<Path>> = LazyLock::new(|| {
    let home_env = env::var_os(DATA_HOME);
    if let Some(home_env_path) = home_env {
        PathBuf::from(home_env_path).join(env!("CARGO_PKG_NAME"))
    } else {
        let mut home_path = data_local_dir().unwrap_or_else(env::temp_dir);
        home_path.push(env!("CARGO_PKG_NAME"));
        home_path
    }
    .into_boxed_path()
});

pub static DATA_BASIS_PATH: LazyLock<Box<Path>> =
    LazyLock::new(|| DATA_HOME_PATH.join("basis_sets").into_boxed_path());

pub fn auto_download_value() -> bool {
    env::var_os(AUTO_DOWNLOAD)
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(AUTO_DOWNLOAD_DEFAULT)
}
