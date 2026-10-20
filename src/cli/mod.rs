pub(crate) mod build_info;
pub(crate) mod color;
pub mod commands;
pub(crate) mod directories;
pub(crate) mod env;
pub(crate) mod exit_status;
pub mod ux;

pub(crate) const BRANDING_NAME: &str = "RustiQ";

use clap::{ColorChoice, Parser};
use commands::Commands;

#[derive(Parser, Debug)]
#[command(name = "rustiq", author, about, long_about = None, disable_version_flag = true, arg_required_else_help = true)]
pub struct Cli {
    /// Print version information
    #[arg(short = 'V', long)]
    pub version: bool,
    /// Include build details when printing the version
    #[arg(short = 'v', long, requires = "version")]
    pub verbose: bool,
    /// Control ANSI colors in terminal output
    #[arg(long, value_enum, global = true, env = "RUSTIQ_COLOR")]
    pub color: Option<ColorChoice>,
    #[command(subcommand)]
    pub command: Option<Commands>,
}
