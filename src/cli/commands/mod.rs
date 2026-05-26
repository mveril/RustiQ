mod basis_command;
mod run_command;
use run_command::RunCommand;
mod geometry_command;
mod runnable;
use basis_command::BasisCommands;
use clap::Subcommand;
use delegate::delegate;
use geometry_command::GeometryCommands;
#[cfg(feature = "online")]
pub(crate) use runnable::AsyncRunnable;
pub(crate) use runnable::{CommandResult, Runnable};

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run a calculation defined in toml format
    Run(RunCommand),
    /// Command to handle basis set cache
    Basis {
        #[command(subcommand)]
        command: BasisCommands,
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
            Commands::Run(command) => command,
            Commands::Basis { command } => command,
            Commands::Geometry { command } => command,
        } {
            fn run(&self) -> CommandResult;
        }
    }
}
