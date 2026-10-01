pub(crate) mod color;
pub mod commands;
pub(crate) mod directories;
pub(crate) mod env;
pub mod ux;
use clap::Parser;
use color::ColorMode;
use commands::Commands;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Control ANSI colors in terminal output
    #[arg(long, value_enum, global = true, env = "RUSTIQ_COLOR")]
    pub color: Option<ColorMode>,
    #[command(subcommand)]
    pub command: Commands,
}
