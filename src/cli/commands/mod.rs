mod artifact_command;
pub(crate) mod artifact_session;
mod basis_command;
pub(crate) mod batch_orchestration;
mod cache_command;
mod init_command;
mod run_command;
pub(crate) use run_command::{validate_run_arguments, RunCommand};
mod geometry_command;
mod runnable;
use basis_command::BasisCommands;
use cache_command::CacheCommands;
use clap::Subcommand;
use delegate::delegate;
use geometry_command::GeometryCommands;
#[cfg(feature = "online")]
pub(crate) use runnable::AsyncRunnable;
pub(crate) use runnable::{CommandResult, Runnable};

#[derive(Subcommand, Debug)]
pub enum Commands {
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
    delegate! {
        to match self {
            Commands::Init(command) => command,
            Commands::Run(command) => command,
            Commands::Basis { command } => command,
            Commands::Cache { command } => command,
            Commands::Artifact { command } => command,
            Commands::Geometry { command } => command,
        } {
            fn run(&self) -> CommandResult;
        }
    }
}
