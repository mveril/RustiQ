use std::io::{self, Write};

use super::{
    metadata::{hyperlink_commit, BuildInfo},
    BUILD_INFO, VERSION,
};
use crate::cli::color;

pub(crate) fn print_version(writer: &mut impl Write, verbose: bool) -> io::Result<()> {
    writeln!(writer, "rustiq {VERSION}")?;
    if verbose {
        print_version_details(
            writer,
            &BUILD_INFO,
            supports_hyperlinks::on(supports_hyperlinks::Stream::Stdout),
        )?;
    }
    Ok(())
}

fn print_version_details(
    writer: &mut impl Write,
    info: &BuildInfo,
    hyperlinks: bool,
) -> io::Result<()> {
    print_source(writer, &info.source, hyperlinks)?;
    print_compiler(writer, &info.compiler, hyperlinks)?;
    print_cpu_compatibility(writer, &info.cpu, hyperlinks)?;
    print_build(writer, &info.build)?;
    section(writer, "NUMERICAL LIBRARIES")?;
    print_backend(
        writer,
        "Linear algebra",
        "nalgebra",
        env!("RUSTIQ_NALGEBRA_VERSION"),
        hyperlinks,
    )?;
    print_backend(
        writer,
        "Parallelism",
        "rayon",
        env!("RUSTIQ_RAYON_VERSION"),
        hyperlinks,
    )?;
    print_backend(
        writer,
        "Special functions",
        "special",
        env!("RUSTIQ_SPECIAL_VERSION"),
        hyperlinks,
    )?;

    section(writer, "CHEMICAL DATA")?;
    print_backend(
        writer,
        "Periodic table",
        "periodic_table",
        env!("RUSTIQ_PERIODIC_TABLE_VERSION"),
        hyperlinks,
    )?;
    print_codata_revision(writer, hyperlinks)?;
    Ok(())
}

/// Report the source of the conversion constant, not a blanket statement
/// about all of RustiQ's scientific data.
fn print_codata_revision(writer: &mut impl Write, hyperlinks: bool) -> io::Result<()> {
    let value = rustiq_core::molecules::BOHR_RADIUS_CODATA_REVISION;
    if hyperlinks {
        let url = rustiq_core::molecules::BOHR_RADIUS_CODATA_URL;
        row_hyperlink(
            writer,
            "Bohr radius",
            format!("\x1b]8;;{url}\x1b\\{value}\x1b]8;;\x1b\\"),
        )
    } else {
        row(writer, "Bohr radius", value)
    }
}

fn print_backend(
    writer: &mut impl Write,
    label: &str,
    name: &str,
    version: &str,
    hyperlinks: bool,
) -> io::Result<()> {
    if version.is_empty() {
        return row(writer, label, format!("{name} (version unknown)"));
    }
    let value = format!("{name} {version}");
    let url = format!("https://crates.io/crates/{name}/{version}");
    if hyperlinks {
        row_hyperlink(
            writer,
            label,
            format!("\x1b]8;;{url}\x1b\\{value}\x1b]8;;\x1b\\"),
        )
    } else {
        row(writer, label, value)
    }
}

fn print_source(
    writer: &mut impl Write,
    source: &super::metadata::Source,
    hyperlinks: bool,
) -> io::Result<()> {
    section(writer, "SOURCE")?;
    if let Some(commit) = source.commit {
        let linked = hyperlinks && is_commit_hash(commit);
        let value = if linked {
            hyperlink_commit(commit, env!("CARGO_PKG_REPOSITORY"))
        } else {
            commit.to_owned()
        };
        row_maybe_hyperlink(writer, "Git commit", value, linked)?;
    } else {
        row(writer, "Git commit", "unknown")?;
    }
    row(writer, "Status", source.state)?;
    Ok(())
}

fn print_cpu_compatibility(
    writer: &mut impl Write,
    cpu: &super::metadata::CpuCompatibility,
    hyperlinks: bool,
) -> io::Result<()> {
    if cpu.architecture.is_none() && cpu.target_cpu.is_none() && cpu.target_features.is_empty() {
        return Ok(());
    }
    section(writer, "CPU TARGET")?;
    if let Some(architecture) = cpu.architecture {
        row(writer, "Architecture", architecture)?;
    }
    if let Some(value) = cpu.target_cpu {
        row(writer, "Target CPU", value)?;
    }
    if !cpu.target_features.is_empty() {
        let features = cpu.target_features.join(", ");
        if hyperlinks && !features.chars().any(char::is_control) {
            row_hyperlink(
                writer,
                "Target features",
                format!(
                    "\x1b]8;;https://doc.rust-lang.org/reference/attributes/codegen.html#the-target_feature-attribute\x1b\\{features}\x1b]8;;\x1b\\"
                ),
            )?;
        } else {
            row(writer, "Target features", features)?;
        }
    }
    Ok(())
}

fn print_compiler(
    writer: &mut impl Write,
    compiler: &super::metadata::Compiler,
    hyperlinks: bool,
) -> io::Result<()> {
    if compiler.rustc.is_none()
        && compiler.channel.is_none()
        && compiler.llvm.is_none()
        && compiler.target.is_none()
    {
        return Ok(());
    }
    section(writer, "COMPILER")?;
    if let Some(value) = compiler.rustc {
        row(writer, "Rust", value)?;
    }
    if let Some(channel) = compiler.channel {
        row(writer, "Channel", channel)?;
        if channel == "nightly" {
            if let Some(commit) = compiler.commit {
                let value = if hyperlinks {
                    hyperlink_commit(commit, "https://github.com/rust-lang/rust")
                } else {
                    commit.to_owned()
                };
                row_maybe_hyperlink(writer, "Rust commit", value, hyperlinks)?;
            }
        }
    }
    if let Some(value) = compiler.llvm {
        row(writer, "LLVM", value)?;
    }
    if let Some(value) = compiler.target {
        row(writer, "Target", value)?;
    }
    Ok(())
}

fn print_build(writer: &mut impl Write, build: &super::metadata::Build) -> io::Result<()> {
    section(writer, "BUILD CONFIGURATION")?;
    if let Some(value) = build.profile {
        row(writer, "Profile", value)?;
    }
    if let Some(value) = build.cargo_optimization {
        row(writer, "Optimization", value)?;
    }
    if let Some(value) = build.debug_information_declared {
        row(writer, "Debug information", format!("{value} (declared)"))?;
    }
    if let Some(value) = build.lto_declared {
        row(writer, "LTO", format!("{value} (declared)"))?;
    }
    if let Some(value) = build.codegen_units_declared {
        row(writer, "Codegen units", format!("{value} (declared)"))?;
    }
    if let Some(value) = build.debug_assertions_declared {
        row(writer, "Debug assertions", format!("{value} (declared)"))?;
    } else {
        row(writer, "Debug assertions", cfg!(debug_assertions))?;
    }
    if let Some(value) = build.overflow_checks_declared {
        row(
            writer,
            "Integer overflow checks",
            format!("{value} (declared)"),
        )?;
    } else {
        row(writer, "Integer overflow checks", "unknown")?;
    }
    if !build.cargo_features.is_empty() {
        row(writer, "Cargo features", build.cargo_features.join(", "))?;
    }
    Ok(())
}

fn section(writer: &mut impl Write, name: &str) -> io::Result<()> {
    writeln!(writer, "\n{}", color::title(name))
}

fn is_commit_hash(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn row_maybe_hyperlink(
    writer: &mut impl Write,
    label: &str,
    value: String,
    hyperlinks: bool,
) -> io::Result<()> {
    if hyperlinks && value.starts_with('\u{1b}') {
        row_hyperlink(writer, label, value)
    } else {
        row(writer, label, value)
    }
}

fn row(writer: &mut impl Write, label: &str, value: impl std::fmt::Display) -> io::Result<()> {
    let value = value.to_string();
    let mut safe = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_control() {
            use std::fmt::Write as _;
            write!(safe, "\\u{{{:04x}}}", u32::from(character))
                .map_err(|_| io::Error::other("failed to format build metadata"))?;
        } else {
            safe.push(character);
        }
    }
    writeln!(writer, "  {label:<26}{safe}")
}

fn row_hyperlink(
    writer: &mut impl Write,
    label: &str,
    value: impl std::fmt::Display,
) -> io::Result<()> {
    writeln!(writer, "  {label:<26}{value}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_features_link_only_when_hyperlinks_are_enabled() {
        let mut cpu = BUILD_INFO.cpu.clone();
        cpu.target_features = vec!["sse2", "avx2"];
        for hyperlinks in [false, true] {
            let mut output = Vec::new();
            print_cpu_compatibility(&mut output, &cpu, hyperlinks).unwrap();
            let output = String::from_utf8(output).unwrap();
            assert!(output.contains("sse2, avx2"));
            assert_eq!(
                output.contains("\x1b]8;;https://doc.rust-lang.org/"),
                hyperlinks
            );
        }
    }

    #[test]
    fn architecture_is_visible_without_explicit_cpu_or_features() {
        let cpu = super::super::metadata::CpuCompatibility {
            architecture: Some("aarch64"),
            target_cpu: None,
            target_features: Vec::new(),
        };
        let mut output = Vec::new();
        print_cpu_compatibility(&mut output, &cpu, false).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("CPU TARGET"));
        assert!(output.contains("Architecture"));
        assert!(output.contains("aarch64"));
        assert!(!output.contains("Target CPU"));
        assert!(!output.contains("Target features"));
    }

    #[test]
    fn chemical_data_respects_hyperlink_settings_and_core_metadata() {
        let revision = rustiq_core::molecules::BOHR_RADIUS_CODATA_REVISION;
        let source = rustiq_core::molecules::BOHR_RADIUS_CODATA_URL;
        for hyperlinks in [false, true] {
            let mut output = Vec::new();
            print_codata_revision(&mut output, hyperlinks).unwrap();
            let output = String::from_utf8(output).unwrap();
            assert!(output.contains("Bohr radius"));
            assert!(output.contains(revision));
            assert_eq!(output.contains(source), hyperlinks);
            assert_eq!(output.contains("\x1b]8;;"), hyperlinks);
        }
    }

    #[test]
    fn declared_checks_report_both_enabled_and_disabled_values() {
        let mut build = BUILD_INFO.build.clone();
        build.debug_assertions_declared = Some("true");
        build.overflow_checks_declared = Some("false");
        let mut output = Vec::new();
        print_build(&mut output, &build).unwrap();
        let output = String::from_utf8(output).unwrap();
        for (label, value) in [
            ("Debug assertions", "true"),
            ("Integer overflow checks", "false"),
        ] {
            let line = output.lines().find(|line| line.contains(label)).unwrap();
            assert!(line.ends_with(&format!("{value} (declared)")));
        }
    }

    #[test]
    fn rust_commit_is_shown_only_for_nightly() {
        let commit = "58cf8f9c35b355199727627d07ae34a4e2846d67";
        for (channel, visible) in [
            ("stable", false),
            ("beta", false),
            ("nightly", true),
            ("dev", false),
        ] {
            let compiler = super::super::metadata::Compiler {
                rustc: Some("1.99.1"),
                channel: Some(channel),
                commit: Some(commit),
                llvm: Some("23.1.1"),
                target: Some("x86_64-unknown-linux-gnu"),
            };
            let mut output = Vec::new();
            print_compiler(&mut output, &compiler, false).unwrap();
            let output = String::from_utf8(output).unwrap();
            assert!(output.contains("Channel"));
            assert_eq!(output.contains(commit), visible, "{channel}");
            assert_eq!(output.contains("Rust commit"), visible, "{channel}");
        }
    }
}
