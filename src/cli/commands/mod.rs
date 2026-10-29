mod artifact_command;
pub(crate) mod artifact_session;
mod basis_command;
pub(crate) mod batch_orchestration;
mod cache_command;
pub(crate) mod calculation_result;
mod docs_command;
#[cfg(any(feature = "manpages", feature = "completions"))]
mod generate_command;
mod init_command;
mod run_command;
pub(crate) use run_command::{validate_run_arguments, RunCommand};
mod geometry_command;
mod runnable;
use basis_command::BasisCommands;
use cache_command::CacheCommands;
use clap::Subcommand;
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
    /// Create a calculation TOML file from an XYZ geometry
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
        match self {
            Self::Docs(command) => command.run(),
            #[cfg(feature = "manpages")]
            Self::Manpages(command) => command.run(),
            #[cfg(feature = "completions")]
            Self::Completions(command) => command.run(),
            Commands::Init(command) => command.run(),
            Commands::Run(command) => command.run(),
            Commands::Basis { command } => command.run(),
            Commands::Cache { command } => command.run(),
            Commands::Artifact { command } => command.run(),
            Commands::Geometry { command } => command.run(),
        }
    }
}
