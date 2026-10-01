use std::{
    ffi::OsString,
    io::IsTerminal,
    sync::atomic::{AtomicBool, Ordering},
};

use clap::ValueEnum;

static COLORS_ENABLED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Default, ValueEnum, PartialEq, Eq)]
pub enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

impl ColorMode {
    pub(crate) fn enabled(self) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => {
                std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
                    && std::io::stdout().is_terminal()
            }
        }
    }

    pub(crate) fn from_process_args(args: impl IntoIterator<Item = OsString>) -> Self {
        let mut args = args.into_iter();
        let mut cli_mode = None;
        while let Some(arg) = args.next() {
            let Some(arg) = arg.to_str() else { continue };
            let value = if arg == "--color" {
                args.next().and_then(|value| value.into_string().ok())
            } else {
                arg.strip_prefix("--color=").map(str::to_owned)
            };
            if let Some(value) = value {
                cli_mode = match value.as_str() {
                    "auto" => Some(Self::Auto),
                    "always" => Some(Self::Always),
                    "never" => Some(Self::Never),
                    _ => None,
                };
            }
        }

        cli_mode
            .or_else(|| {
                std::env::var("RUSTIQ_COLOR")
                    .ok()
                    .and_then(|value| match value.as_str() {
                        "auto" => Some(Self::Auto),
                        "always" => Some(Self::Always),
                        "never" => Some(Self::Never),
                        _ => None,
                    })
            })
            .unwrap_or(Self::Auto)
    }

    pub(crate) fn clap_choice(self) -> clap::ColorChoice {
        match self {
            Self::Auto => clap::ColorChoice::Auto,
            Self::Always => clap::ColorChoice::Always,
            Self::Never => clap::ColorChoice::Never,
        }
    }
}

pub(crate) fn configure(mode: ColorMode) {
    COLORS_ENABLED.store(mode.enabled(), Ordering::Relaxed);
}

pub(crate) fn enabled() -> bool {
    COLORS_ENABLED.load(Ordering::Relaxed)
}

pub(crate) fn paint(text: impl std::fmt::Display, code: &str) -> String {
    if enabled() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

pub(crate) fn title(text: impl std::fmt::Display) -> String {
    paint(text, "1;36")
}

pub(crate) fn success(text: impl std::fmt::Display) -> String {
    paint(text, "32")
}

pub(crate) fn error(text: impl std::fmt::Display) -> String {
    paint(text, "1;31")
}

pub(crate) fn value(text: impl std::fmt::Display) -> String {
    paint(text, "1;33")
}
