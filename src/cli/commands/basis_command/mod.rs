mod download_command;
mod list_command;
mod remove_command;
use clap::Subcommand;
use download_command::DownloadCommand;
use list_command::ListCommand;
use remove_command::RemoveCommand;

use super::Runable;

#[derive(Subcommand, Debug)]
pub enum BasisCommands {
    Download(DownloadCommand),
    List(ListCommand),
    Remove(RemoveCommand),
}

impl Runable for BasisCommands {
    fn run(&self) {
        match self {
            BasisCommands::Download(cmd) => cmd.run(),
            BasisCommands::List(cmd) => cmd.run(),
            BasisCommands::Remove(cmd) => cmd.run(),
        }
    }
}
