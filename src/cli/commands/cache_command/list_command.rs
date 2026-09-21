use miette::IntoDiagnostic;
use rustiq_core::persistence::{EriCache, EriCacheEntry};
use tabled::{Table, Tabled};

use crate::cli::commands::{CommandResult, Runnable};

#[derive(clap::Args, Debug)]
pub struct ListCommand {
    /// List entries and verify payload size, digest, and supported NPY layout.
    /// The `verified` status means all of those checks completed successfully.
    /// Cache root defaults to the RustiQ platform cache directory.
    #[arg(long, value_name = "DIR")]
    cache_dir: Option<std::path::PathBuf>,
}

impl Runnable for ListCommand {
    fn run(&self) -> CommandResult {
        let root = self
            .cache_dir
            .clone()
            .unwrap_or_else(crate::cli::directories::cache_path);
        let cache = EriCache::new(root);
        // A read-only cache remains inspectable even when aliases cannot be assigned.
        let _ = cache.assign_missing_names();
        println!("{}", render_entries(cache.entries().into_diagnostic()?));
        Ok(())
    }
}

#[derive(Tabled)]
#[tabled(rename_all = "UPPERCASE")]
struct CacheRow {
    name: String,
    fingerprint: String,
    size: String,
    status: &'static str,
}

fn render_entries(entries: Vec<EriCacheEntry>) -> String {
    if entries.is_empty() {
        return "No cache entries found.".to_owned();
    }
    Table::new(entries.into_iter().map(|entry| {
        CacheRow {
            name: entry.name.unwrap_or_else(|| "-".to_owned()),
            fingerprint: entry.fingerprint,
            size: entry
                .payload_size
                .map(|size| bytesize::ByteSize(size).to_string())
                .unwrap_or_else(|| "unknown".to_owned()),
            status: if entry.verified {
                "verified"
            } else {
                "invalid"
            },
        }
    }))
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_includes_full_fingerprints_and_missing_metadata() {
        let output = render_entries(vec![
            EriCacheEntry {
                name: Some("calm-photon".into()),
                fingerprint: "a".repeat(64),
                payload_size: Some(1024),
                verified: true,
            },
            EriCacheEntry {
                name: None,
                fingerprint: "b".repeat(64),
                payload_size: None,
                verified: false,
            },
        ]);
        for expected in [
            "NAME",
            "FINGERPRINT",
            "SIZE",
            "STATUS",
            "calm-photon",
            "verified",
            "invalid",
            "unknown",
            "-",
        ] {
            assert!(output.contains(expected), "missing {expected}: {output}");
        }
        assert!(output.contains(&"a".repeat(64)));
        assert!(output.contains(&bytesize::ByteSize(1024).to_string()));
        assert_eq!(render_entries(Vec::new()), "No cache entries found.");
    }
}
