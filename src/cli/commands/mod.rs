mod basis_command;
mod run_command;
use run_command::RunCommand;
mod runable;
use basis_command::BasisCommands;
use clap::Subcommand;
pub(crate) use runable::Runable;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run a calculation defined in toml format
    Run(RunCommand),
    /// Command to handle basis set cache
    Basis {
        #[command(subcommand)]
        command: BasisCommands,
    },
}

impl Runable for Commands {
    fn run(&self) {
        match &self {
            Commands::Run(run_command) => run_command.run(),
            Commands::Basis { command } => command.run(),
        }
    }
}
