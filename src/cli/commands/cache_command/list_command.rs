use miette::IntoDiagnostic;
use rustiq_core::persistence::EriCache;

use crate::cli::commands::{CommandResult, Runnable};

#[derive(clap::Args, Debug)]
pub struct ListCommand {
    /// Cache root directory; defaults to the RustiQ platform cache directory.
    #[arg(long, value_name = "DIR")]
    cache_dir: Option<std::path::PathBuf>,
}

impl Runnable for ListCommand {
    fn run(&self) -> CommandResult {
        let root = self
            .cache_dir
            .clone()
            .unwrap_or_else(crate::cli::directories::cache_path);
        for entry in EriCache::new(root).entries().into_diagnostic()? {
            let size = entry
                .payload_size
                .map(|size| size.to_string())
                .unwrap_or_else(|| "unknown".to_owned());
            let status = if entry.valid_manifest {
                "valid"
            } else {
                "invalid"
            };
            println!("{}\t{}\t{}", entry.fingerprint, size, status);
        }
        Ok(())
    }
}
