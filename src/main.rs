use cli::Cli;
mod basis;
mod cli;
mod eri;
mod hf;
mod math_utils;
mod molecules;
mod runfile;
use clap::Parser;
use cli::commands::Runnable;
#[cfg(test)]
pub(crate) mod test_utils;

fn main() -> cli::commands::CommandResult {
    let app: Cli = Cli::parse();
    app.command.run()
}
