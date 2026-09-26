mod basis_command;
mod cache_command;
mod init_command;
mod run_command;
use run_command::RunCommand;
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
            Commands::Geometry { command } => command,
        } {
            fn run(&self) -> CommandResult;
        }
    }
}
