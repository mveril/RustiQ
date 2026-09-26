pub mod commands;
pub(crate) mod directories;
pub(crate) mod env;
pub mod ux;
use clap::Parser;
use commands::Commands;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}
