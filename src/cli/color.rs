use std::{
    env::{var, var_os},
    ffi::OsString,
    fmt::{self, Display},
    io::{self, IsTerminal},
    sync::atomic::{AtomicU8, Ordering},
};

use anstream::{AutoStream, ColorChoice as StreamColorChoice};
use anstyle::{AnsiColor, Style};
use clap::ColorChoice;

static COLOR_CHOICE: AtomicU8 = AtomicU8::new(2);

#[cfg(test)]
thread_local! {
    static TEST_COLOR_CHOICE: std::cell::Cell<Option<ColorChoice>> = const {
        std::cell::Cell::new(None)
    };
}

#[cfg(test)]
pub(crate) fn with_test_color<T>(mode: ColorChoice, render: impl FnOnce() -> T) -> T {
    struct Restore(Option<ColorChoice>);

    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_COLOR_CHOICE.set(self.0);
        }
    }

    let _restore = Restore(TEST_COLOR_CHOICE.replace(Some(mode)));
    render()
}

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
    #[cfg(test)]
    let choice = TEST_COLOR_CHOICE.get().map_or_else(
        || COLOR_CHOICE.load(Ordering::Relaxed),
        |mode| match mode {
            ColorChoice::Always => 0,
            ColorChoice::Never => 1,
            ColorChoice::Auto => 2,
        },
    );
    #[cfg(not(test))]
    let choice = COLOR_CHOICE.load(Ordering::Relaxed);

    match choice {
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

fn stream_choice(stream: OutputStream) -> StreamColorChoice {
    if enabled_for(stream) {
        StreamColorChoice::Always
    } else {
        StreamColorChoice::Never
    }
}

pub(crate) fn stdout() -> AutoStream<io::Stdout> {
    AutoStream::new(io::stdout(), stream_choice(OutputStream::Stdout))
}

pub(crate) struct Styled<T> {
    value: T,
    style: Style,
    enabled: bool,
}

impl<T: Display> Display for Styled<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.enabled {
            write!(
                formatter,
                "{}{}{}",
                self.style,
                self.value,
                self.style.render_reset()
            )
        } else {
            self.value.fmt(formatter)
        }
    }
}

fn paint<T: Display>(value: T, style: Style) -> Styled<T> {
    Styled {
        value,
        style,
        enabled: enabled(),
    }
}

pub(crate) fn title<T: Display>(value: T) -> Styled<T> {
    paint(
        value,
        Style::new().bold().fg_color(Some(AnsiColor::Cyan.into())),
    )
}

pub(crate) fn success<T: Display>(value: T) -> Styled<T> {
    paint(value, Style::new().fg_color(Some(AnsiColor::Green.into())))
}

pub(crate) fn error<T: Display>(value: T) -> Styled<T> {
    paint(
        value,
        Style::new().bold().fg_color(Some(AnsiColor::Red.into())),
    )
}

pub(crate) fn value<T: Display>(value: T) -> Styled<T> {
    paint(
        value,
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

    #[test]
    fn test_color_override_is_thread_local_and_restored_after_panic() {
        assert_eq!(TEST_COLOR_CHOICE.get(), None);
        with_test_color(ColorChoice::Never, || {
            std::thread::spawn(|| {
                assert_eq!(TEST_COLOR_CHOICE.get(), None);
                with_test_color(ColorChoice::Always, || assert!(enabled()));
                assert_eq!(TEST_COLOR_CHOICE.get(), None);
            })
            .join()
            .unwrap();
            assert!(!enabled());

            let result = std::panic::catch_unwind(|| {
                with_test_color(ColorChoice::Always, || {
                    assert!(enabled());
                    panic!("exercise color override restoration");
                });
            });
            assert!(result.is_err());
            assert!(!enabled());
        });
        assert_eq!(TEST_COLOR_CHOICE.get(), None);
    }

    #[test]
    fn styled_display_defers_rendering_until_formatting() {
        let style = Style::new().bold().fg_color(Some(AnsiColor::Cyan.into()));
        let plain = Styled {
            value: "RustiQ",
            style,
            enabled: false,
        };
        assert_eq!(plain.to_string(), "RustiQ");

        let styled = Styled {
            value: "RustiQ",
            style,
            enabled: true,
        }
        .to_string();
        assert!(styled.contains("\x1b["));
        assert!(styled.contains("RustiQ"));
    }
}
