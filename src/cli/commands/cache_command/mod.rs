mod list_command;
mod remove_command;

use clap::Subcommand;
use delegate::delegate;
use list_command::ListCommand;
use remove_command::RemoveCommand;

use super::{CommandResult, Runnable};

#[derive(Subcommand, Debug)]
pub enum CacheCommands {
    /// List published AO ERI cache entries
    List(ListCommand),
    /// Remove one or all AO ERI cache entries
    Remove(RemoveCommand),
}

impl Runnable for CacheCommands {
    delegate! {
        to match self {
            CacheCommands::List(command) => command,
            CacheCommands::Remove(command) => command,
        } {
            fn run(&self) -> CommandResult;
        }
    }
}
