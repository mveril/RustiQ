// User text and filesystem paths require careful handling at the CLI boundary.
#![deny(clippy::string_slice, clippy::path_buf_push_overwrite)]

use std::env;

use clap::{CommandFactory, FromArgMatches};
use cli::{commands::Runnable, Cli};

mod cli;
mod config;
mod runfile;

fn main() -> miette::Result<()> {
    let mode = cli::color::from_process_args(env::args_os().skip(1));
    let command = Cli::command().color(mode);
    let matches = command.get_matches();
    let app = Cli::from_arg_matches(&matches).expect("clap command matches Cli definition");
    let mode = app.color.unwrap_or(mode);
    cli::color::configure(mode);
    let color_enabled = cli::color::enabled_for(cli::color::OutputStream::Stderr);
    miette::set_hook(Box::new(move |_| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .color(color_enabled)
                .build(),
        )
    }))
    .map_err(|error| miette::miette!("failed to install diagnostic hook: {error}"))?;
    app.command.run()
}
