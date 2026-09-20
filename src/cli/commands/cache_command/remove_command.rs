use clap::{ArgAction, ArgGroup};
use miette::{miette, IntoDiagnostic};
use rustiq_core::persistence::EriCache;

use crate::cli::commands::{CommandResult, Runnable};

#[derive(clap::Args, Debug)]
#[command(group(
    ArgGroup::new("target")
        .required(true)
        .multiple(false)
        .args(["fingerprint", "all"])
))]
pub struct RemoveCommand {
    /// Full lowercase SHA-256 fingerprint shown by `rustiq cache list`.
    #[arg(value_name = "FINGERPRINT")]
    fingerprint: Option<String>,
    /// Remove every published AO ERI cache entry.
    #[arg(long, action = ArgAction::SetTrue)]
    all: bool,
    /// Cache root directory; defaults to the RustiQ platform cache directory.
    #[arg(long, value_name = "DIR")]
    cache_dir: Option<std::path::PathBuf>,
}

impl Runnable for RemoveCommand {
    fn run(&self) -> CommandResult {
        let root = self
            .cache_dir
            .clone()
            .unwrap_or_else(crate::cli::env::eri_cache_path);
        let cache = EriCache::new(root);
        if self.all {
            cache.remove_all().into_diagnostic()?;
            return Ok(());
        }
        let fingerprint = self.fingerprint.as_deref().expect("clap requires a target");
        if !cache.remove(fingerprint).into_diagnostic()? {
            return Err(miette!(
                "No AO ERI cache entry exists for fingerprint '{fingerprint}'"
            ));
        }
        Ok(())
    }
}
