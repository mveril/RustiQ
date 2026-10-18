use std::io::{self, Write};

pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

fn known(value: &'static str) -> &'static str {
    match value {
        "" | "VERGEN_IDEMPOTENT_OUTPUT" => "unknown",
        _ => value,
    }
}

pub(crate) fn git_commit() -> &'static str {
    let injected = env!("RUSTIQ_BUILD_GIT_COMMIT");
    known(if injected.is_empty() {
        env!("VERGEN_GIT_SHA")
    } else {
        injected
    })
}

fn git_state() -> &'static str {
    let injected = env!("RUSTIQ_BUILD_GIT_STATE");
    if !injected.is_empty() {
        return injected;
    }
    // An injected commit may describe sources unrelated to the local checkout.
    if !env!("RUSTIQ_BUILD_GIT_COMMIT").is_empty() {
        return "unknown";
    }
    match env!("VERGEN_GIT_DIRTY") {
        "true" => "dirty",
        "false" => "clean",
        _ => "unknown",
    }
}

pub(crate) fn git_line() -> String {
    format_git_line(git_commit(), git_state())
}

fn format_git_line(commit: &str, state: &str) -> String {
    if state == "clean" || (commit == "unknown" && state == "unknown") {
        format!("Git commit: {commit}")
    } else {
        format!("Git commit: {commit} ({state})")
    }
}

pub(crate) fn terminal_git_line() -> String {
    let commit = git_commit();
    let linked_commit = if supports_hyperlinks::on(supports_hyperlinks::Stream::Stdout) {
        hyperlink_commit(commit)
    } else {
        commit.to_owned()
    };
    format_git_line(&linked_commit, git_state())
}

fn hyperlink_commit(commit: &str) -> String {
    if !matches!(commit.len(), 40 | 64) || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return commit.to_owned();
    }

    let repository = env!("CARGO_PKG_REPOSITORY").trim_end_matches('/');
    if !repository.starts_with("https://") {
        return commit.to_owned();
    }

    let url_commit = commit.to_ascii_lowercase();
    format!("\x1b]8;;{repository}/commit/{url_commit}\x1b\\{commit}\x1b]8;;\x1b\\")
}

pub(crate) fn print_version(writer: &mut impl Write, verbose: bool) -> io::Result<()> {
    writeln!(writer, "rustiq {VERSION}")?;
    if verbose {
        writeln!(writer, "{}", terminal_git_line())?;
        for (label, value) in [
            ("Target", env!("VERGEN_CARGO_TARGET_TRIPLE")),
            ("Rust compiler", env!("VERGEN_RUSTC_SEMVER")),
            ("Rust compiler commit", env!("VERGEN_RUSTC_COMMIT_HASH")),
            ("Optimization level", env!("VERGEN_CARGO_OPT_LEVEL")),
            ("Debug information", env!("VERGEN_CARGO_DEBUG")),
        ] {
            writeln!(writer, "{label}: {}", known(value))?;
        }
        let raw_features = env!("VERGEN_CARGO_FEATURES");
        let mut features: Vec<_> = raw_features
            .split(',')
            .filter(|feature| !feature.is_empty())
            .map(|feature| feature.replace('_', "-"))
            .collect();
        features.sort();
        let features = features.join(", ");
        writeln!(
            writer,
            "Features: {}",
            if features.is_empty() {
                "none"
            } else if known(raw_features) == "unknown" {
                "unknown"
            } else {
                &features
            }
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::format_git_line;

    #[test]
    fn clean_is_implicit_and_other_states_remain_explicit() {
        assert_eq!(format_git_line("abc", "clean"), "Git commit: abc");
        assert_eq!(format_git_line("abc", "dirty"), "Git commit: abc (dirty)");
        assert_eq!(
            format_git_line("abc", "unknown"),
            "Git commit: abc (unknown)"
        );
        assert_eq!(format_git_line("unknown", "unknown"), "Git commit: unknown");
    }
}
