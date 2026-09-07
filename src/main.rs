use clap::Parser;
use cli::{commands::Runnable, Cli};

mod cli;

fn main() -> miette::Result<()> {
    let app: Cli = Cli::parse();
    app.command.run()
}
