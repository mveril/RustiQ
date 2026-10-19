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
    print_compiler(writer, &info.compiler)?;
    print_build(writer, &info.build)?;
    Ok(())
}

fn print_source(
    writer: &mut impl Write,
    source: &super::metadata::Source,
    hyperlinks: bool,
) -> io::Result<()> {
    if source.commit.is_none() && source.digest.is_none() && source.cargo_lock_digest.is_none() {
        return Ok(());
    }
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
    if let Some(value) = source.digest {
        row(writer, "Source SHA-256", short_digest(value))?;
    }
    if let Some(value) = source.cargo_lock_digest {
        row(writer, "Cargo.lock SHA-256", short_digest(value))?;
    }
    Ok(())
}

fn print_compiler(writer: &mut impl Write, compiler: &super::metadata::Compiler) -> io::Result<()> {
    if compiler.rustc.is_none() && compiler.llvm.is_none() && compiler.target.is_none() {
        return Ok(());
    }
    section(writer, "COMPILER")?;
    if let Some(value) = compiler.rustc {
        row(writer, "Rust", value)?;
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
    let any = build.profile.is_some()
        || build.cargo_optimization.is_some()
        || build.debug_information_declared.is_some()
        || build.lto_declared.is_some()
        || build.codegen_units_declared.is_some()
        || build.target_cpu.is_some()
        || !build.target_features.is_empty()
        || !build.cargo_features.is_empty();
    if !any {
        return Ok(());
    }
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
    if let Some(value) = build.target_cpu {
        row(writer, "Target CPU", value)?;
    }
    if !build.target_features.is_empty() {
        row(writer, "Target features", build.target_features.join(", "))?;
    }
    if !build.cargo_features.is_empty() {
        row(writer, "Cargo features", build.cargo_features.join(", "))?;
    }
    Ok(())
}

fn section(writer: &mut impl Write, name: &str) -> io::Result<()> {
    writeln!(writer, "\n{}", color::title(name))
}

fn short_digest(value: &str) -> String {
    if value.len() > 12 {
        format!("{}…", value.get(..12).unwrap_or(value))
    } else {
        value.to_owned()
    }
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
