pub(crate) mod color;
pub mod commands;
pub(crate) mod directories;
pub(crate) mod env;
pub mod ux;

pub(crate) const BRANDING_NAME: &str = "RustiQ";

use clap::{ColorChoice, Parser};
use commands::Commands;

#[derive(Parser, Debug)]
#[command(name = "rustiq", author, version, about, long_about = None)]
pub struct Cli {
    /// Control ANSI colors in terminal output
    #[arg(long, value_enum, global = true, env = "RUSTIQ_COLOR")]
    pub color: Option<ColorChoice>,
    #[command(subcommand)]
    pub command: Commands,
}
