mod artifact_command;
pub(crate) mod artifact_session;
mod basis_command;
pub(crate) mod batch_orchestration;
mod bundle_selection;
mod cache_command;
pub(crate) mod calculation_result;
mod docs_command;
#[cfg(any(feature = "manpages", feature = "completions"))]
mod generate_command;
mod init_command;
mod post_scf;
mod run_command;
pub(crate) use run_command::{validate_run_arguments, RunCommand};
mod geometry_command;
mod runnable;
use basis_command::BasisCommands;
use cache_command::CacheCommands;
use clap::Subcommand;

#[derive(clap::Args, Debug, Clone, Copy, Default)]
pub(crate) struct OutputOptions {
    /// Replace an existing output file.
    #[arg(short, long)]
    pub(crate) force: bool,
}
use geometry_command::GeometryCommands;
#[cfg(feature = "online")]
pub(crate) use runnable::AsyncRunnable;
pub(crate) use runnable::{CommandResult, Runnable};

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Open the installed manual, or the online manual for Cargo installations
    Docs(docs_command::DocsCommand),
    /// Generate manual pages in a directory
    #[cfg(feature = "manpages")]
    Manpages(generate_command::ManpagesCommand),
    /// Write a shell completion script to standard output
    #[cfg(feature = "completions")]
    Completions(generate_command::CompletionsCommand),
    /// Create a calculation input from an XYZ geometry (TOML by default)
    Init(init_command::InitCommand),
    /// Run a calculation defined in toml format
    Run(RunCommand),
    /// Command to handle basis set cache
    Basis {
        #[command(subcommand)]
        command: BasisCommands,
    },
    /// Inspect and remove deterministic scientific artifacts
    Cache {
        #[command(subcommand)]
        command: CacheCommands,
    },
    /// Inspect portable scientific artifact bundles
    Artifact {
        #[command(subcommand)]
        command: artifact_command::ArtifactCommands,
    },
    /// Inspect and transform molecular geometry files
    Geometry {
        #[command(subcommand)]
        command: GeometryCommands,
    },
}

impl Runnable for Commands {
    fn run(&self) -> CommandResult {
        let (result, code) = match self {
            Self::Docs(command) => (command.run(), "rustiq::docs::failed"),
            #[cfg(feature = "manpages")]
            Self::Manpages(command) => (command.run(), "rustiq::manpages::failed"),
            #[cfg(feature = "completions")]
            Self::Completions(command) => (command.run(), "rustiq::completions::failed"),
            Commands::Init(command) => (command.run(), "rustiq::init::failed"),
            Commands::Run(command) => (command.run(), "rustiq::run::failed"),
            Commands::Basis { command } => (command.run(), "rustiq::basis::failed"),
            Commands::Cache { command } => (command.run(), "rustiq::cache::failed"),
            Commands::Artifact { command } => (command.run(), "rustiq::artifact::failed"),
            Commands::Geometry { command } => (command.run(), "rustiq::geometry::failed"),
        };
        result.map_err(|report| super::ux::diagnostics::with_code(report, code))
    }
}
