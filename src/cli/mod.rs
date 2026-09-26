pub mod commands;
pub mod ux;
pub(crate) mod directories;
pub(crate) mod env;
use clap::Parser;
use commands::Commands;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}
