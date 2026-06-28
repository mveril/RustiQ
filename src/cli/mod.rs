pub mod commands;
pub mod ux;
use clap::Parser;
use commands::Commands;
pub(crate) mod env;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}
