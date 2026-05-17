use cli::Cli;
mod basis;
mod runfile;
mod cli;
mod eri;
mod hf;
mod math_utils;
mod molecules;
use clap::Parser;
use cli::commands::Runable;
#[cfg(test)]
pub(crate) mod test_utils;

fn main() {
    let app: Cli = Cli::parse();
    app.command.run()
}
