use cli::Cli;
mod basis;
mod cli;
mod env;
mod eri;
mod hf;
mod math_utils;
mod molecules;
#[allow(dead_code)]
mod mp2;
mod runfile;
use clap::Parser;
use cli::commands::Runnable;
#[cfg(test)]
pub(crate) mod test_utils;

fn main() -> miette::Result<()> {
    let app: Cli = Cli::parse();
    app.command.run()
}
