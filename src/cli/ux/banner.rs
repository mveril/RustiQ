use figlet_rs::FIGfont;
use rand::RngExt;

const BANNER_STYLE_COUNT: u8 = 4;

pub(crate) fn print_startup_banner() {
    let package_name = env!("CARGO_PKG_NAME");
    let package_version = env!("CARGO_PKG_VERSION");
    let banner = render_package_name(package_name);
    let style = rand::rng().random_range(0..BANNER_STYLE_COUNT);

    match style {
        0 => print_plain_banner(&banner, package_version),
        1 => print_framed_banner(&banner, package_version),
        2 => print_rule_banner(&banner, package_version),
        _ => print_compact_banner(&banner, package_version),
    }

    println!();
}

fn render_package_name(package_name: &str) -> String {
    match FIGfont::standard() {
        Ok(font) => font
            .convert(package_name)
            .map(|figure| figure.to_string())
            .unwrap_or_else(|| package_name.to_string()),
        Err(_) => package_name.to_string(),
    }
}

fn print_plain_banner(banner: &str, package_version: &str) {
    print!("{banner}");
    println!("v{package_version}");
}

fn print_framed_banner(banner: &str, package_version: &str) {
    let width = banner_width(banner).max(package_version.len() + 2);
    let border = "=".repeat(width);

    println!("{border}");
    print!("{banner}");
    println!("{:^width$}", format!("v{package_version}"));
    println!("{border}");
}

fn print_rule_banner(banner: &str, package_version: &str) {
    print!("{banner}");
    println!(
        "{}",
        "-".repeat(banner_width(banner).max(package_version.len() + 2))
    );
    println!("v{package_version}");
}

fn print_compact_banner(banner: &str, package_version: &str) {
    for line in banner.lines() {
        println!("  {line}");
    }

    println!("  v{package_version}");
}

fn banner_width(banner: &str) -> usize {
    banner.lines().map(str::len).max().unwrap_or_default()
}
