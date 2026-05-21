mod download_command;
mod list_command;
mod remove_command;
use clap::Subcommand;
use download_command::DownloadCommand;
use list_command::ListCommand;
use remove_command::RemoveCommand;
mod import_command;
use import_command::ImportCommand;

use super::Runnable;

#[derive(Subcommand, Debug)]
pub enum BasisCommands {
    Download(DownloadCommand),
    Import(ImportCommand),
    List(ListCommand),
    Remove(RemoveCommand),
}

impl Runnable for BasisCommands {
    fn run(&self) {
        match self {
            BasisCommands::Download(cmd) => cmd.run(),
            BasisCommands::Import(cmd) => cmd.run(),
            BasisCommands::List(cmd) => cmd.run(),
            BasisCommands::Remove(cmd) => cmd.run(),
        }
    }
}
