use std::io::{self, Write};

use rand::RngExt;
use rusty_figlet::FigletBuilder;

use crate::cli::{build_info, color, BRANDING_NAME};

const BANNER_STYLE_COUNT: u8 = 4;
const STANDARD_FONT: &[u8] = include_bytes!("../../../assets/fonts/standard.flf");

pub(crate) fn print_startup_banner() -> io::Result<()> {
    let package_version = build_info::VERSION;
    let banner = render_branding_name(BRANDING_NAME);
    let style = rand::rng().random_range(0..BANNER_STYLE_COUNT);
    let mut stdout = color::stdout().lock();

    match style {
        0 => print_plain_banner(&mut stdout, &banner, package_version)?,
        1 => print_framed_banner(&mut stdout, &banner, package_version)?,
        2 => print_rule_banner(&mut stdout, &banner, package_version)?,
        _ => print_compact_banner(&mut stdout, &banner, package_version)?,
    }

    writeln!(stdout)
}

fn render_branding_name(branding_name: &str) -> String {
    FigletBuilder::new()
        .font_bytes(STANDARD_FONT)
        .build()
        .and_then(|font| font.render(branding_name))
        .map_or_else(|_| branding_name.to_string(), |banner| banner.to_string())
}

fn print_plain_banner(
    writer: &mut impl Write,
    banner: &str,
    package_version: &str,
) -> io::Result<()> {
    write!(writer, "{}", color::title(banner))?;
    writeln!(writer, "{}", color::value(format!("v{package_version}")))?;
    writeln!(writer, "{}", color::value(build_info::git_line()))
}

fn print_framed_banner(
    writer: &mut impl Write,
    banner: &str,
    package_version: &str,
) -> io::Result<()> {
    let commit = build_info::git_line();
    let width = banner_width(banner)
        .max(package_version.len() + 2)
        .max(commit.len());
    let border = "=".repeat(width);

    writeln!(writer, "{}", color::title(&border))?;
    write!(writer, "{}", color::title(banner))?;
    writeln!(
        writer,
        "{}",
        color::value(format!("{:^width$}", format!("v{package_version}")))
    )?;
    writeln!(writer, "{}", color::value(format!("{commit:^width$}")))?;
    writeln!(writer, "{}", color::title(&border))
}

fn print_rule_banner(
    writer: &mut impl Write,
    banner: &str,
    package_version: &str,
) -> io::Result<()> {
    write!(writer, "{}", color::title(banner))?;
    writeln!(
        writer,
        "{}",
        "-".repeat(
            banner_width(banner)
                .max(package_version.len() + 2)
                .max(build_info::git_line().len())
        )
    )?;
    writeln!(writer, "{}", color::value(format!("v{package_version}")))?;
    writeln!(writer, "{}", color::value(build_info::git_line()))
}

fn print_compact_banner(
    writer: &mut impl Write,
    banner: &str,
    package_version: &str,
) -> io::Result<()> {
    for line in banner.lines() {
        writeln!(writer, "  {}", color::title(line))?;
    }

    writeln!(writer, "  {}", color::value(format!("v{package_version}")))?;
    writeln!(writer, "  {}", color::value(build_info::git_line()))
}

fn banner_width(banner: &str) -> usize {
    banner.lines().map(str::len).max().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_places_the_commit_immediately_below_the_version() {
        let banner = render_branding_name(BRANDING_NAME);
        for style in [
            print_plain_banner,
            print_framed_banner,
            print_rule_banner,
            print_compact_banner,
        ] {
            let mut output = Vec::new();
            color::with_test_color(clap::ColorChoice::Never, || {
                style(&mut output, &banner, build_info::VERSION)
            })
            .expect("writing a banner to a Vec succeeds");
            let text = String::from_utf8(output).expect("banner is UTF-8");
            let lines: Vec<_> = text.lines().collect();
            let index = lines
                .iter()
                .position(|line| line.trim() == format!("v{}", build_info::VERSION))
                .expect("version line");
            assert_eq!(lines[index + 1].trim(), build_info::git_line());
            assert!(!text.contains('\u{1b}'));
        }
    }

    #[test]
    fn frame_fits_the_commit() {
        let mut output = Vec::new();
        color::with_test_color(clap::ColorChoice::Never, || {
            print_framed_banner(&mut output, "RustiQ\n", build_info::VERSION)
        })
        .expect("writing a banner to a Vec succeeds");
        let text = String::from_utf8(output).expect("banner is UTF-8");
        let lines: Vec<_> = text.lines().collect();
        assert!(lines[0].len() >= build_info::git_line().len());
        assert_eq!(lines[0].len(), lines[2].len());
        assert_eq!(lines[0].len(), lines[3].len());
        assert_eq!(lines[0], lines[4]);
    }
}
