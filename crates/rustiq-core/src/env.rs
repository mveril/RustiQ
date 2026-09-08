use std::{
    env,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use const_format::formatcp;
use dirs::data_local_dir;

pub const DATA_HOME: &str = "RUSTIQ_DATA_HOME";
pub const BASIS_HOME: &str = "RUSTIQ_DATA_BASIS";

pub const USER_AGENT: &str = formatcp!(
    "{}/{} ({}; {}; +{})",
    "RustiQ",
    env!("CARGO_PKG_VERSION"),
    std::env::consts::OS,
    std::env::consts::ARCH,
    env!("CARGO_PKG_REPOSITORY"),
);

pub static DATA_HOME_PATH: LazyLock<Box<Path>> = LazyLock::new(|| {
    let home_env = env::var_os(DATA_HOME);
    if let Some(home_env_path) = home_env {
        PathBuf::from(home_env_path).join("RustiQ")
    } else {
        let mut home_path = data_local_dir().unwrap_or_else(env::temp_dir);
        home_path.push("RustiQ");
        home_path
    }
    .into_boxed_path()
});

pub static DATA_BASIS_PATH: LazyLock<Box<Path>> = LazyLock::new(|| {
    env::var_os(BASIS_HOME)
        .map(PathBuf::from)
        .unwrap_or_else(|| DATA_HOME_PATH.join("basis_sets"))
        .into_boxed_path()
});
