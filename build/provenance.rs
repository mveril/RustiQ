use std::{error::Error, path::Path};
use vergen_gix::{Emitter, Gix};

pub fn git_instructions(path: &Path) -> Result<Emitter, Box<dyn Error>> {
    let mut git = Gix::builder().sha(false).dirty(false).build();
    git.at_path(path.to_path_buf());
    let mut emitter = Emitter::default();
    emitter.default_on_error().add_instructions(&git)?;
    Ok(emitter)
}

pub fn validate_override(name: &str, value: &str) -> Result<(), Box<dyn Error>> {
    if name == "RUSTIQ_BUILD_GIT_COMMIT"
        && !value.is_empty()
        && value != "unknown"
        && !(matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err("RUSTIQ_BUILD_GIT_COMMIT must be a full Git hash or unknown".into());
    }
    if name == "RUSTIQ_BUILD_GIT_STATE" && !matches!(value, "" | "clean" | "dirty" | "unknown") {
        return Err("RUSTIQ_BUILD_GIT_STATE must be clean, dirty, or unknown".into());
    }
    if name == "RUSTIQ_BUILD_PROFILE"
        && !value.is_empty()
        && !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(
            "RUSTIQ_BUILD_PROFILE must contain only ASCII letters, digits, '-' or '_'".into(),
        );
    }
    if name == "RUSTIQ_BUILD_TARGET_CPU" && !valid_flag_value(value) {
        return Err("RUSTIQ_BUILD_TARGET_CPU contains unsupported characters".into());
    }
    if name == "RUSTIQ_BUILD_SYSTEM"
        && !value.is_empty()
        && !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err("RUSTIQ_BUILD_SYSTEM contains unsupported characters".into());
    }
    Ok(())
}

pub fn validate_vergen_git_override(name: &str, value: &str) -> Result<(), Box<dyn Error>> {
    let is_valid_sha = value.is_empty()
        || matches!(value, "unknown" | "VERGEN_IDEMPOTENT_OUTPUT")
        || (matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit()));
    if name == "VERGEN_GIT_SHA" && !is_valid_sha {
        return Err("VERGEN_GIT_SHA must be a full Git hash or unknown".into());
    }
    if name == "VERGEN_GIT_DIRTY"
        && !value.is_empty()
        && !matches!(value, "true" | "false" | "VERGEN_IDEMPOTENT_OUTPUT")
    {
        return Err("VERGEN_GIT_DIRTY must be true or false".into());
    }
    Ok(())
}

fn valid_flag_value(value: &str) -> bool {
    value.is_empty()
        || value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'+' | b'.'))
}

pub fn target_cpu_from_flags(flags: &str) -> String {
    if flags
        .chars()
        .any(|character| character.is_control() && character != '\u{1f}')
    {
        return String::new();
    }
    let parts: Vec<&str> = if flags.contains('\u{1f}') {
        flags.split('\u{1f}').collect()
    } else {
        flags.split_whitespace().collect()
    };
    let mut value = None;
    let mut index = 0;
    while index < parts.len() {
        let part = parts[index];
        if part == "-C" {
            if let Some(option) = parts
                .get(index + 1)
                .and_then(|next| next.strip_prefix("target-cpu="))
            {
                value = Some(option);
            }
            index += 1;
        } else if let Some(option) = part.strip_prefix("-Ctarget-cpu=") {
            value = Some(option);
        } else if let Some(option) = part.strip_prefix("target-cpu=") {
            value = Some(option);
        }
        index += 1;
    }
    value
        .filter(|candidate| valid_flag_value(candidate))
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_cpu_reads_encoded_and_split_codegen_arguments() {
        assert_eq!(
            target_cpu_from_flags("-C\u{1f}target-cpu=x86-64-v3"),
            "x86-64-v3"
        );
        assert_eq!(target_cpu_from_flags("-Ctarget-cpu=native"), "native");
        assert_eq!(
            target_cpu_from_flags("-C target-cpu=x86-64-v3"),
            "x86-64-v3"
        );
        assert_eq!(target_cpu_from_flags("-Ctarget-cpu=bad\nvalue"), "");
        assert_eq!(target_cpu_from_flags("-Copt-level=3"), "");
    }
}
