use std::path::PathBuf;

use clap::{ArgAction, ArgGroup};
use miette::{miette, IntoDiagnostic};
use rustiq_core::persistence::EriCache;

use crate::cli::{commands::{CommandResult, Runnable}, directories::cache_path};

#[derive(clap::Args, Debug)]
#[command(group(
    ArgGroup::new("target")
        .required(true)
        .multiple(false)
        .args(["entry", "all"])
))]
pub struct RemoveCommand {
    /// Persistent name or full lowercase SHA-256 fingerprint shown by `cache list`.
    #[arg(value_name = "NAME|FINGERPRINT")]
    entry: Option<String>,
    /// Remove every published AO ERI cache entry.
    #[arg(long, action = ArgAction::SetTrue)]
    all: bool,
    /// Cache root directory; defaults to the RustiQ platform cache directory.
    #[arg(long, value_name = "DIR")]
    cache_dir: Option<PathBuf>,
}

impl Runnable for RemoveCommand {
    fn run(&self) -> CommandResult {
        let root = self
            .cache_dir
            .clone()
            .unwrap_or_else(cache_path);
        let cache = EriCache::new(root);
        if self.all {
            cache.remove_all().into_diagnostic()?;
            return Ok(());
        }
        let target = self.entry.as_deref().expect("clap requires a target");
        if !cache.remove_named(target).into_diagnostic()? {
            return Err(miette!("No AO ERI cache entry exists for '{target}'"));
        }
        Ok(())
    }
}
