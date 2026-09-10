use clap::Parser;
use cli::{commands::Runnable, Cli};

mod cli;
mod runfile;

fn main() -> miette::Result<()> {
    let app: Cli = Cli::parse();
    app.command.run()
}
