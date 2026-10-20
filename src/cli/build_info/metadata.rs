use std::sync::LazyLock;

pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");
const UNKNOWN: &str = "VERGEN_IDEMPOTENT_OUTPUT";

#[derive(Clone, Debug)]
pub(crate) struct BuildInfo {
    pub source: Source,
    pub compiler: Compiler,
    pub cpu: CpuCompatibility,
    pub build: Build,
}

#[derive(Clone, Debug)]
pub(crate) struct Source {
    pub commit: Option<&'static str>,
    pub state: &'static str,
}

#[derive(Clone, Debug)]
pub(crate) struct Compiler {
    pub rustc: Option<&'static str>,
    pub channel: Option<&'static str>,
    pub commit: Option<&'static str>,
    pub llvm: Option<&'static str>,
    pub target: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub(crate) struct CpuCompatibility {
    pub architecture: Option<&'static str>,
    pub target_cpu: Option<&'static str>,
    pub target_features: Vec<&'static str>,
}

#[derive(Clone, Debug)]
pub(crate) struct Build {
    pub profile: Option<&'static str>,
    pub cargo_optimization: Option<&'static str>,
    pub debug_information_declared: Option<&'static str>,
    pub lto_declared: Option<&'static str>,
    pub codegen_units_declared: Option<&'static str>,
    pub debug_assertions_declared: Option<&'static str>,
    pub overflow_checks_declared: Option<&'static str>,
    pub cargo_features: Vec<String>,
}

pub(crate) static BUILD_INFO: LazyLock<BuildInfo> = LazyLock::new(|| {
    let target_features = target_feature_values(env!("RUSTIQ_TARGET_FEATURES"));
    BuildInfo {
        source: Source {
            commit: optional_git(env!("RUSTIQ_BUILD_GIT_COMMIT"), env!("VERGEN_GIT_SHA")),
            state: git_state(),
        },
        compiler: Compiler {
            rustc: optional(env!("VERGEN_RUSTC_SEMVER")),
            channel: optional(env!("VERGEN_RUSTC_CHANNEL")),
            commit: optional_git("", env!("VERGEN_RUSTC_COMMIT_HASH")),
            llvm: optional(env!("VERGEN_RUSTC_LLVM_VERSION")),
            target: optional(env!("VERGEN_CARGO_TARGET_TRIPLE")),
        },
        cpu: CpuCompatibility {
            architecture: optional(env!("RUSTIQ_TARGET_ARCH")),
            target_cpu: optional(env!("RUSTIQ_BUILD_TARGET_CPU")),
            target_features,
        },
        build: Build {
            profile: optional(env!("RUSTIQ_BUILD_PROFILE")),
            cargo_optimization: optional(env!("VERGEN_CARGO_OPT_LEVEL")),
            debug_information_declared: optional(env!("RUSTIQ_DEBUG_INFO_DECLARED")),
            lto_declared: optional(env!("RUSTIQ_LTO_DECLARED")),
            codegen_units_declared: optional(env!("RUSTIQ_CODEGEN_UNITS_DECLARED")),
            debug_assertions_declared: optional(env!("RUSTIQ_DEBUG_ASSERTIONS_DECLARED")),
            overflow_checks_declared: optional(env!("RUSTIQ_OVERFLOW_CHECKS_DECLARED")),
            cargo_features: cargo_feature_values(env!("VERGEN_CARGO_FEATURES")),
        },
    }
});

fn optional(value: &'static str) -> Option<&'static str> {
    if value.is_empty() || value == UNKNOWN {
        None
    } else {
        Some(value)
    }
}

fn optional_git(injected: &'static str, discovered: &'static str) -> Option<&'static str> {
    let value = if injected.is_empty() {
        discovered
    } else {
        injected
    };
    if matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(value)
    } else {
        None
    }
}

fn git_state() -> &'static str {
    let injected = env!("RUSTIQ_BUILD_GIT_STATE");
    if !injected.is_empty() {
        return injected;
    }
    if !env!("RUSTIQ_BUILD_GIT_COMMIT").is_empty() {
        return "unknown";
    }
    match env!("VERGEN_GIT_DIRTY") {
        "true" => "dirty",
        "false" => "clean",
        _ => "unknown",
    }
}

fn feature_values(value: &'static str) -> Vec<&'static str> {
    if value.is_empty() || value == UNKNOWN {
        return Vec::new();
    }
    let mut features: Vec<_> = value
        .split(',')
        .filter(|feature| !feature.is_empty())
        .collect();
    features.sort_unstable();
    features.dedup();
    features
}

fn target_feature_values(value: &'static str) -> Vec<&'static str> {
    feature_values(value)
        .into_iter()
        .filter(|feature| *feature != "crt-static")
        .collect()
}

fn cargo_feature_values(value: &'static str) -> Vec<String> {
    let mut features: Vec<_> = feature_values(value)
        .into_iter()
        .filter(|feature| *feature != "default")
        .map(|feature| feature.replace('_', "-"))
        .collect();
    features.sort_unstable();
    features.dedup();
    features
}

pub(crate) fn hyperlink_commit(commit: &str, repository: &str) -> String {
    hyperlink_commit_as(commit, commit, repository)
}

pub(crate) fn hyperlink_commit_as(commit: &str, label: &str, repository: &str) -> String {
    if !matches!(commit.len(), 40 | 64) || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return commit.to_owned();
    }
    let repository = repository.trim_end_matches('/');
    if !repository.starts_with("https://")
        || !repository.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return commit.to_owned();
    }
    let commit = commit.to_ascii_lowercase();
    format!("\x1b]8;;{repository}/commit/{commit}\x1b\\{label}\x1b]8;;\x1b\\")
}

#[cfg(test)]
mod tests {
    use super::{cargo_feature_values, target_feature_values};

    #[test]
    fn cargo_feature_list_hides_the_default_marker_and_is_sorted() {
        assert_eq!(
            cargo_feature_values("online,default,bench_support,online"),
            ["bench-support", "online"]
        );
        assert_eq!(cargo_feature_values("default"), Vec::<String>::new());
    }

    #[test]
    fn target_features_omit_the_linking_mode_marker() {
        assert!(target_feature_values("avx2,crt-static").contains(&"avx2"));
        assert!(!target_feature_values("avx2,crt-static").contains(&"crt-static"));
    }
}
