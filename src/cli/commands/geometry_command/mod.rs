mod info_command;
use clap::Subcommand;
use info_command::InfoCommand;

use super::{CommandResult, Runnable};

#[derive(Subcommand, Debug)]
pub enum GeometryCommands {
    Info(InfoCommand),
}

impl Runnable for GeometryCommands {
    fn run(&self) -> CommandResult {
        match self {
            GeometryCommands::Info(cmd) => cmd.run(),
        }
    }
}
