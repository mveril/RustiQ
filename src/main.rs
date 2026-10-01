use clap::{CommandFactory, FromArgMatches};
use cli::{commands::Runnable, Cli};

mod cli;
mod runfile;

fn main() -> miette::Result<()> {
    let mode = cli::color::from_process_args(std::env::args_os().skip(1));
    let command = Cli::command().color(mode);
    let matches = command.get_matches();
    let app = Cli::from_arg_matches(&matches).expect("clap command matches Cli definition");
    let mode = app.color.unwrap_or(mode);
    cli::color::configure(mode);
    let color_enabled = cli::color::enabled();
    let _ = miette::set_hook(Box::new(move |_| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .color(color_enabled)
                .build(),
        )
    }));
    app.command.run()
}
