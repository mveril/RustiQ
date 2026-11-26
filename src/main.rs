// User text and filesystem paths require careful handling at the CLI boundary.
#![deny(clippy::string_slice, clippy::path_buf_push_overwrite)]

use std::{env, io::Write, process::ExitCode};

use clap::{CommandFactory, FromArgMatches};
use cli::{commands::Runnable, ux::diagnostics::CodedHandler, Cli};
use miette::IntoDiagnostic;

mod cli;
mod config;
mod runfile;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let status = cli::exit_status::for_error(&error);
            let prefix = if error.severity() == Some(miette::Severity::Warning) {
                "Warning"
            } else {
                "Error"
            };
            if writeln!(std::io::stderr().lock(), "{prefix}: {error:?}").is_err() {
                return ExitCode::FAILURE;
            }
            status
        }
    }
}

fn run() -> miette::Result<()> {
    let mode = cli::color::from_process_args(env::args_os().skip(1));
    let mut command = Cli::command().color(mode);
    let matches = match command.try_get_matches_from_mut(env::args_os()) {
        Ok(matches) => matches,
        Err(error) => exit_clap(&error),
    };
    if let Err(error) = cli::commands::validate_run_arguments(&matches) {
        exit_clap(&error);
    }
    let app = match Cli::from_arg_matches(&matches) {
        Ok(app) => app,
        Err(error) => exit_clap(&error),
    };
    // Use Clap's resolved global option before returning for --version too.
    let mode = app.color.unwrap_or(mode);
    cli::color::configure(mode);
    let color_enabled = cli::color::enabled_for(cli::color::OutputStream::Stderr);
    miette::set_hook(Box::new(move |_| {
        Box::new(CodedHandler(
            miette::MietteHandlerOpts::new()
                .color(color_enabled)
                .build(),
        ))
    }))
    .map_err(|error| {
        miette::miette!(
            code = "rustiq::cli::diagnostic_hook",
            "failed to install diagnostic hook: {error}"
        )
    })?;
    if app.version {
        if app.command.is_some() {
            exit_clap(&command.error(
                clap::error::ErrorKind::ArgumentConflict,
                "--version cannot be combined with a subcommand",
            ));
        }
        return cli::build_info::print_version(&mut std::io::stdout().lock(), app.verbose)
            .into_diagnostic();
    }
    let Some(subcommand) = app.command else {
        exit_clap(&command.error(
            clap::error::ErrorKind::MissingSubcommand,
            "a subcommand is required",
        ));
    };
    subcommand.run()
}

fn exit_clap(error: &clap::Error) -> ! {
    if error.use_stderr() {
        drop(writeln!(
            std::io::stderr().lock(),
            "rustiq::cli::invalid_arguments"
        ));
    }
    error.exit()
}
