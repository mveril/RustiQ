use std::io::{self, Write};

use figlet_rs::FIGlet;
use rand::RngExt;

use crate::cli::{color, BRANDING_NAME};

const BANNER_STYLE_COUNT: u8 = 4;

pub(crate) fn print_startup_banner() -> io::Result<()> {
    let package_version = env!("CARGO_PKG_VERSION");
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
    match FIGlet::standard() {
        Ok(font) => font
            .convert(branding_name)
            .map_or_else(|| branding_name.to_string(), |figure| figure.to_string()),
        Err(_) => branding_name.to_string(),
    }
}

fn print_plain_banner(
    writer: &mut impl Write,
    banner: &str,
    package_version: &str,
) -> io::Result<()> {
    write!(writer, "{}", color::title(banner))?;
    writeln!(writer, "{}", color::value(format!("v{package_version}")))
}

fn print_framed_banner(
    writer: &mut impl Write,
    banner: &str,
    package_version: &str,
) -> io::Result<()> {
    let width = banner_width(banner).max(package_version.len() + 2);
    let border = "=".repeat(width);

    writeln!(writer, "{}", color::title(&border))?;
    write!(writer, "{}", color::title(banner))?;
    writeln!(
        writer,
        "{}",
        color::value(format!("{:^width$}", format!("v{package_version}")))
    )?;
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
        "-".repeat(banner_width(banner).max(package_version.len() + 2))
    )?;
    writeln!(writer, "{}", color::value(format!("v{package_version}")))
}

fn print_compact_banner(
    writer: &mut impl Write,
    banner: &str,
    package_version: &str,
) -> io::Result<()> {
    for line in banner.lines() {
        writeln!(writer, "  {}", color::title(line))?;
    }

    writeln!(writer, "  {}", color::value(format!("v{package_version}")))
}

fn banner_width(banner: &str) -> usize {
    banner.lines().map(str::len).max().unwrap_or_default()
}
