mod display;
mod metadata;

pub(crate) use display::print_version;
pub(crate) use metadata::{BUILD_INFO, VERSION};

pub(crate) fn git_commit() -> Option<&'static str> {
    BUILD_INFO.source.commit
}

pub(crate) fn banner_build_line(link: bool) -> String {
    format_banner_build_line(git_commit(), BUILD_INFO.source.state, link)
}

pub(crate) fn git_line() -> String {
    banner_build_line(false)
}

fn format_banner_build_line(commit: Option<&str>, state: &str, link: bool) -> String {
    let Some(commit) = commit.filter(|commit| {
        matches!(commit.len(), 40 | 64) && commit.bytes().all(|byte| byte.is_ascii_hexdigit())
    }) else {
        return if state == "dirty" {
            "Build: unknown (dirty)".to_owned()
        } else {
            "Build: unknown".to_owned()
        };
    };
    let short = commit.get(..12).unwrap_or(commit);
    let displayed = if link {
        metadata::hyperlink_commit_as(commit, short, env!("CARGO_PKG_REPOSITORY"))
    } else {
        short.to_owned()
    };
    match state {
        "clean" => format!("Build: {displayed}"),
        "dirty" | "unknown" => format!("Build: {displayed} ({state})"),
        _ => "Build: unknown".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::format_banner_build_line;

    #[test]
    fn banners_abbreviate_valid_hashes_and_never_abbreviate_unknown_provenance() {
        let commit = "ef7cb4c164fa14cec31287f892941bbe4078ed37";
        assert_eq!(
            format_banner_build_line(Some(commit), "clean", false),
            "Build: ef7cb4c164fa"
        );
        assert_eq!(
            format_banner_build_line(Some(commit), "dirty", false),
            "Build: ef7cb4c164fa (dirty)"
        );
        assert_eq!(
            format_banner_build_line(None, "unknown", false),
            "Build: unknown"
        );
        assert_eq!(
            format_banner_build_line(None, "dirty", false),
            "Build: unknown (dirty)"
        );
        assert_eq!(
            format_banner_build_line(Some("short"), "dirty", false),
            "Build: unknown (dirty)"
        );
    }

    #[test]
    fn abbreviated_hyperlink_uses_the_full_commit_as_its_target() {
        let commit = "ef7cb4c164fa14cec31287f892941bbe4078ed37";
        let line = format_banner_build_line(Some(commit), "clean", true);
        assert!(line.contains(&format!("/commit/{commit}")));
        assert!(line.contains("\\ef7cb4c164fa\u{1b}]8;;\u{1b}\\"));
    }
}
