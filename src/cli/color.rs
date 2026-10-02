use std::{
    env::{var, var_os},
    ffi::OsString,
    fmt::Display,
    io::{self, IsTerminal},
    sync::atomic::{AtomicU8, Ordering},
};

use anstyle::{AnsiColor, Style};
use clap::ColorChoice;

static COLOR_CHOICE: AtomicU8 = AtomicU8::new(2);

#[derive(Clone, Copy)]
pub(crate) enum OutputStream {
    Stdout,
    Stderr,
}

pub(crate) fn from_process_args(args: impl IntoIterator<Item = OsString>) -> ColorChoice {
    let cli_mode = cli_mode_from_args(args);
    cli_mode
        .or_else(|| {
            var("RUSTIQ_COLOR")
                .ok()
                .and_then(|value| value.parse::<ColorChoice>().ok())
        })
        .unwrap_or_default()
}

fn cli_mode_from_args(args: impl IntoIterator<Item = OsString>) -> Option<ColorChoice> {
    let mut args = args.into_iter();
    let mut cli_mode = None;
    while let Some(arg) = args.next() {
        let Some(arg) = arg.to_str() else { continue };
        if arg == "--" {
            break;
        }
        let value = if arg == "--color" {
            let Some(value) = args.next() else {
                break;
            };
            if value == "--" {
                break;
            }
            value.into_string().ok()
        } else {
            arg.strip_prefix("--color=").map(str::to_owned)
        };
        if let Some(value) = value {
            cli_mode = value.parse::<ColorChoice>().ok();
        }
    }

    cli_mode
}

pub(crate) fn configure(mode: ColorChoice) {
    COLOR_CHOICE.store(
        match mode {
            ColorChoice::Always => 0,
            ColorChoice::Never => 1,
            ColorChoice::Auto => 2,
        },
        Ordering::Relaxed,
    );
}

pub(crate) fn enabled_for(stream: OutputStream) -> bool {
    match COLOR_CHOICE.load(Ordering::Relaxed) {
        0 => true,
        1 => false,
        _ => {
            var_os("NO_COLOR").is_none_or(|value| value.is_empty())
                && match stream {
                    OutputStream::Stdout => io::stdout().is_terminal(),
                    OutputStream::Stderr => io::stderr().is_terminal(),
                }
        }
    }
}

pub(crate) fn enabled() -> bool {
    enabled_for(OutputStream::Stdout)
}

pub(crate) fn paint(text: impl Display, style: Style) -> String {
    if enabled() {
        format!("{style}{text}{}", style.render_reset())
    } else {
        text.to_string()
    }
}

pub(crate) fn title(text: impl Display) -> String {
    paint(
        text,
        Style::new().bold().fg_color(Some(AnsiColor::Cyan.into())),
    )
}

pub(crate) fn success(text: impl Display) -> String {
    paint(text, Style::new().fg_color(Some(AnsiColor::Green.into())))
}

pub(crate) fn error(text: impl Display) -> String {
    paint(
        text,
        Style::new().bold().fg_color(Some(AnsiColor::Red.into())),
    )
}

pub(crate) fn value(text: impl Display) -> String {
    paint(
        text,
        Style::new().bold().fg_color(Some(AnsiColor::Yellow.into())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn early_color_scan_stops_at_argument_delimiter() {
        for args in [
            vec!["run", "--", "--color=always"],
            vec!["--color", "--", "--color=always"],
        ] {
            let args = args.into_iter().map(OsString::from);
            assert_eq!(cli_mode_from_args(args), None);
        }
    }
}
