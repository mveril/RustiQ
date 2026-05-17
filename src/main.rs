use cli::Cli;
mod basis;
mod cli;
mod eri;
mod hf;
mod math_utils;
mod molecules;
mod runfile;
use clap::Parser;
use cli::commands::Runable;
#[cfg(test)]
pub(crate) mod test_utils;

fn main() {
    let app: Cli = Cli::parse();
    app.command.run()
}
