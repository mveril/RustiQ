pub mod commands;
pub mod ux;
use clap::{CommandFactory, Parser};
use commands::Commands;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}
