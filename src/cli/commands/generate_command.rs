use clap::{Command, CommandFactory};
use miette::IntoDiagnostic;

use super::{CommandResult, Runnable};
use crate::cli::Cli;

/// Describe the installed application without the optional generation commands.
fn application_command() -> Command {
    let cli = Cli::command();
    // Clap cannot remove subcommands. Reuse the derived arguments and command
    // definitions rather than hiding generators (some shells complete hidden commands).
    Command::new("rustiq")
        .bin_name("rustiq")
        .about(cli.get_about().cloned().unwrap_or_default())
        .author(env!("CARGO_PKG_AUTHORS"))
        .version(env!("CARGO_PKG_VERSION"))
        .disable_version_flag(true)
        .arg_required_else_help(true)
        .args(cli.get_arguments().cloned())
        .subcommands(
            cli.get_subcommands()
                .filter(|command| !matches!(command.get_name(), "manpages" | "completions"))
                .cloned(),
        )
}

#[cfg(feature = "manpages")]
#[derive(clap::Args, Debug)]
pub struct ManpagesCommand {
    /// Destination directory (created if needed; existing generated pages are replaced)
    directory: std::path::PathBuf,
}

#[cfg(feature = "manpages")]
impl Runnable for ManpagesCommand {
    fn run(&self) -> CommandResult {
        std::fs::create_dir_all(&self.directory).into_diagnostic()?;
        let mut command = application_command().disable_help_subcommand(true);
        command.build();
        write_manpages(command, &self.directory).into_diagnostic()
    }
}

#[cfg(feature = "manpages")]
fn write_manpages(command: Command, directory: &std::path::Path) -> std::io::Result<()> {
    use std::io::Write;

    for subcommand in command.get_subcommands().filter(|sub| !sub.is_hide_set()) {
        write_manpages(subcommand.clone(), directory)?;
    }
    let man = clap_mangen::Man::new(command);
    let mut file =
        std::io::BufWriter::new(std::fs::File::create(directory.join(man.get_filename()))?);
    man.render(&mut file)?;
    // Keep the URL visible even in man viewers without hyperlink support.
    let mut references = clap_mangen::roff::Roff::default();
    references.control("SH", ["SEE ALSO"]);
    references.control("UR", ["https://mveril.github.io/RustiQ/"]);
    references.text([clap_mangen::roff::roman("RustiQ manual")]);
    references.control("UE", std::iter::empty::<&str>());
    references.to_writer(&mut file)?;
    file.flush()
}

#[cfg(feature = "completions")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum Shell {
    Bash,
    Zsh,
    Fish,
    Elvish,
    Powershell,
    Nushell,
}

#[cfg(feature = "completions")]
impl Shell {
    fn from_executable(executable: &std::ffi::OsStr) -> Option<Self> {
        let name = std::path::Path::new(executable)
            .file_name()?
            .to_str()?
            .to_ascii_lowercase();
        let name = name.strip_suffix(".exe").unwrap_or(&name);
        let name = name.trim_start_matches(['-', '.']);
        let name = name.strip_suffix("-wrapped").unwrap_or(name);
        match name {
            "bash" => Some(Self::Bash),
            "zsh" => Some(Self::Zsh),
            "fish" => Some(Self::Fish),
            "elvish" => Some(Self::Elvish),
            "powershell" | "powershell_ise" | "pwsh" => Some(Self::Powershell),
            "nu" | "nushell" => Some(Self::Nushell),
            _ => None,
        }
    }

    fn current() -> Option<Self> {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

        let mut system = System::new();
        let refresh = ProcessRefreshKind::nothing()
            .without_tasks()
            .with_exe(UpdateKind::Always);
        if let Ok(pid) = sysinfo::get_current_pid() {
            system.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, refresh);
            if let Some(parent) = system.process(pid).and_then(sysinfo::Process::parent) {
                system.refresh_processes_specifics(
                    ProcessesToUpdate::Some(&[parent]),
                    true,
                    refresh,
                );
                if let Some(process) = system.process(parent) {
                    let shell = process
                        .exe()
                        .and_then(|path| Self::from_executable(path.as_os_str()))
                        .or_else(|| Self::from_executable(process.name()));
                    if shell.is_some() {
                        return shell;
                    }
                }
            }
        }
        std::env::var_os("SHELL")
            .as_deref()
            .and_then(Self::from_executable)
    }
}

#[cfg(feature = "completions")]
#[derive(clap::Args, Debug)]
pub struct CompletionsCommand {
    /// Target shell (defaults to the calling shell, then SHELL)
    #[arg(value_enum)]
    shell: Option<Shell>,
}

#[cfg(feature = "completions")]
impl Runnable for CompletionsCommand {
    fn run(&self) -> CommandResult {
        use clap_complete::Generator;
        use std::io::Write;

        let shell = self.shell.or_else(Shell::current).ok_or_else(|| {
            miette::miette!(
                "could not detect a supported shell; specify bash, zsh, fish, elvish, powershell, or nushell"
            )
        })?;
        let mut command = application_command();
        command.build();
        let mut output = std::io::stdout().lock();
        match shell {
            Shell::Bash => clap_complete::Shell::Bash.try_generate(&command, &mut output),
            Shell::Zsh => clap_complete::Shell::Zsh.try_generate(&command, &mut output),
            Shell::Fish => clap_complete::Shell::Fish.try_generate(&command, &mut output),
            Shell::Elvish => clap_complete::Shell::Elvish.try_generate(&command, &mut output),
            Shell::Powershell => {
                clap_complete::Shell::PowerShell.try_generate(&command, &mut output)
            }
            Shell::Nushell => clap_complete_nushell::Nushell.try_generate(&command, &mut output),
        }
        .into_diagnostic()?;
        output.flush().into_diagnostic()
    }
}

#[cfg(all(test, feature = "completions"))]
mod tests {
    use super::Shell;

    #[test]
    fn recognizes_shell_executables_and_nix_wrappers() {
        for (executable, shell) in [
            ("/bin/bash", Some(Shell::Bash)),
            ("-zsh", Some(Shell::Zsh)),
            ("/bin/fish", Some(Shell::Fish)),
            ("elvish", Some(Shell::Elvish)),
            ("pwsh.exe", Some(Shell::Powershell)),
            ("PowerShell.EXE", Some(Shell::Powershell)),
            ("nu", Some(Shell::Nushell)),
            ("/nix/store/example/bin/.nu-wrapped", Some(Shell::Nushell)),
            ("/bin/sh", None),
        ] {
            assert_eq!(Shell::from_executable(executable.as_ref()), shell);
        }
    }
}
