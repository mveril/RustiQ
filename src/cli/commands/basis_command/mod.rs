#[cfg(feature = "online")]
mod download_command;
mod list_command;
mod remove_command;
use clap::Subcommand;
use delegate::delegate;
#[cfg(feature = "online")]
use download_command::DownloadCommand;
use list_command::ListCommand;
use remove_command::RemoveCommand;
mod import_command;
use import_command::ImportCommand;

use super::{CommandResult, Runnable};

#[derive(Subcommand, Debug)]
pub enum BasisCommands {
    #[cfg(feature = "online")]
    Download(DownloadCommand),
    Import(ImportCommand),
    List(ListCommand),
    Remove(RemoveCommand),
}

impl Runnable for BasisCommands {
    delegate! {
        to match self {
            #[cfg(feature = "online")]
            BasisCommands::Download(command) => command,
            BasisCommands::Import(command) => command,
            BasisCommands::List(command) => command,
            BasisCommands::Remove(command) => command,
        } {
            fn run(&self) -> CommandResult;
        }
    }
}
