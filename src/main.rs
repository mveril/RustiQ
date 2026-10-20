// User text and filesystem paths require careful handling at the CLI boundary.
#![deny(clippy::string_slice, clippy::path_buf_push_overwrite)]

use std::{env, io::Write, process::ExitCode};

use clap::{CommandFactory, FromArgMatches};
use cli::{commands::Runnable, Cli};
use miette::IntoDiagnostic;

mod cli;
mod config;
mod runfile;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let status = cli::exit_status::for_error(&error);
            if writeln!(std::io::stderr().lock(), "Error: {error:?}").is_err() {
                return ExitCode::FAILURE;
            }
            status
        }
    }
}

fn run() -> miette::Result<()> {
    let mode = cli::color::from_process_args(env::args_os().skip(1));
    let mut command = Cli::command().color(mode);
    let matches = command.get_matches_mut();
    if let Err(error) = cli::commands::validate_run_arguments(&matches) {
        error.exit();
    }
    let app = Cli::from_arg_matches(&matches).expect("clap command matches Cli definition");
    // Use Clap's resolved global option before returning for --version too.
    let mode = app.color.unwrap_or(mode);
    cli::color::configure(mode);
    if app.version {
        if app.command.is_some() {
            command
                .error(
                    clap::error::ErrorKind::ArgumentConflict,
                    "--version cannot be combined with a subcommand",
                )
                .exit();
        }
        return cli::build_info::print_version(&mut std::io::stdout().lock(), app.verbose)
            .into_diagnostic();
    }
    let Some(subcommand) = app.command else {
        command
            .error(
                clap::error::ErrorKind::MissingSubcommand,
                "a subcommand is required",
            )
            .exit();
    };
    let color_enabled = cli::color::enabled_for(cli::color::OutputStream::Stderr);
    miette::set_hook(Box::new(move |_| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .color(color_enabled)
                .build(),
        )
    }))
    .map_err(|error| miette::miette!("failed to install diagnostic hook: {error}"))?;
    subcommand.run()
}
