use std::{env, path::PathBuf};

use clap::Args;
use miette::{Context, IntoDiagnostic};

use super::{CommandResult, Runnable};

const ONLINE_MANUAL: &str = "https://mveril.github.io/RustiQ/";

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("failed to open the manual at {target}")]
#[diagnostic(help(
    "On a headless machine, run `rustiq docs --print-path` and copy the path or URL to a browser."
))]
struct BrowserError {
    target: String,
    #[source]
    source: std::io::Error,
}

#[derive(Args, Debug)]
pub struct DocsCommand {
    /// Open the online manual even when a local manual is installed
    #[arg(long)]
    web: bool,
    /// Print the local index path (or online URL) without opening a browser
    #[arg(long)]
    print_path: bool,
}

impl DocsCommand {
    fn target(&self) -> miette::Result<std::ffi::OsString> {
        if self.web {
            return Ok(ONLINE_MANUAL.into());
        }
        let executable = env::current_exe()
            .into_diagnostic()
            .wrap_err("failed to locate the RustiQ executable")?;
        let directory = executable.parent().ok_or_else(|| {
            miette::miette!("the RustiQ executable has no installation directory")
        })?;
        let mut candidates = vec![directory.join("manual/html")];
        if let Some(prefix) = directory.parent() {
            candidates.push(prefix.join("share/doc/rustiq/manual/html"));
        }
        for manual in candidates {
            // A present directory with a missing index is a damaged installation,
            // not a reason to silently switch to the network.
            if manual.try_exists().into_diagnostic()? {
                let index: PathBuf = manual.join("index.html");
                if !index.is_file() {
                    return Err(miette::miette!(
                        "installed manual is incomplete: {} is missing; reinstall the complete package or use `rustiq docs --web`",
                        index.display()
                    ));
                }
                return Ok(index.into_os_string());
            }
        }
        Ok(ONLINE_MANUAL.into())
    }
}

impl Runnable for DocsCommand {
    fn run(&self) -> CommandResult {
        let target = self.target()?;
        if self.print_path {
            println!("{}", target.to_string_lossy());
            return Ok(());
        }
        // BROWSER is an executable name/path, passed directly without a shell.
        let result = match env::var_os("BROWSER").filter(|value| !value.is_empty()) {
            Some(browser) => std::process::Command::new(browser)
                .arg(&target)
                .status()
                .and_then(|status| {
                    if status.success() {
                        Ok(())
                    } else {
                        Err(std::io::Error::other(format!(
                            "browser exited with {status}"
                        )))
                    }
                }),
            None => webbrowser::open(&target.to_string_lossy()),
        };
        result.map_err(|source| {
            miette::Report::new(BrowserError {
                target: target.to_string_lossy().into_owned(),
                source,
            })
        })
    }
}
