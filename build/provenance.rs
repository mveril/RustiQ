use std::{
    error::Error,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use vergen_gix::{Emitter, Gix};

const FINGERPRINT_PATHS: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "build.rs",
    "build",
    "src",
    "crates/rustiq-core/Cargo.toml",
    "crates/rustiq-core/build.rs",
    "crates/rustiq-core/build",
    "crates/rustiq-core/src",
    "assets",
    ".cargo/config",
    ".cargo/config.toml",
];

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

#[cfg(not(test))]
pub fn emit_fingerprints(root: &Path) -> Result<(), Box<dyn Error>> {
    let lock_digest = cargo_lock_digest(root)?;
    println!(
        "cargo:rustc-env=RUSTIQ_CARGO_LOCK_SHA256={}",
        lock_digest.as_deref().unwrap_or("unknown")
    );

    let source_digest = source_fingerprint(root)?;
    println!("cargo:rustc-env=RUSTIQ_SOURCE_SHA256={source_digest}");
    Ok(())
}

pub fn cargo_lock_digest(root: &Path) -> Result<Option<String>, Box<dyn Error>> {
    let lock = root.join("Cargo.lock");
    let watched = if lock.exists() { &lock } else { root };
    println!("cargo:rerun-if-changed={}", watched.display());
    digest_file(&lock)
}

pub fn source_fingerprint(root: &Path) -> Result<String, Box<dyn Error>> {
    let mut paths = Vec::new();
    for relative in FINGERPRINT_PATHS {
        let path = root.join(relative);
        if path.is_dir() {
            println!("cargo:rerun-if-changed={}", path.display());
            collect_files(root, &path, &mut paths)?;
        } else if path.is_file() {
            println!("cargo:rerun-if-changed={}", path.display());
            paths.push(path);
        } else {
            let parent = path.parent().unwrap_or(root);
            println!("cargo:rerun-if-changed={}", parent.display());
        }
    }
    let mut named = paths
        .into_iter()
        .map(|path| Ok((relative_name(root, &path)?, path)))
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    named.sort_by(|left, right| left.0.cmp(&right.0));
    named.dedup_by(|left, right| left.0 == right.0);

    let mut hash = Sha256::new();
    hash.update(b"rustiq-source-fingerprint-v1\0");
    for (name, path) in named {
        let name = name.as_bytes();
        hash.update((name.len() as u64).to_be_bytes());
        hash.update(name);
        let mut file = fs::File::open(&path)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;
        hash.update((contents.len() as u64).to_be_bytes());
        hash.update(contents);
    }
    Ok(to_hex(&hash.finalize()))
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

fn digest_file(path: &Path) -> Result<Option<String>, Box<dyn Error>> {
    if !path.is_file() {
        return Ok(None);
    }
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(Some(to_hex(&hash.finalize())))
}

fn collect_files(
    root: &Path,
    directory: &Path,
    paths: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_dir() {
            println!("cargo:rerun-if-changed={}", path.display());
            collect_files(root, &path, paths)?;
        } else if kind.is_file() {
            println!("cargo:rerun-if-changed={}", path.display());
            // Require portable relative names so equivalent trees hash identically
            // on Unix and Windows.
            relative_name(root, &path)?;
            paths.push(path);
        } else if kind.is_symlink() {
            return Err(format!(
                "symlinks are outside the source fingerprint scope: {}",
                path.display()
            )
            .into());
        }
    }
    Ok(())
}

fn relative_name(root: &Path, path: &Path) -> Result<String, Box<dyn Error>> {
    let relative = path.strip_prefix(root)?;
    let components = relative
        .components()
        .map(|component| {
            component
                .as_os_str()
                .to_str()
                .ok_or_else(|| "source paths must be UTF-8".into())
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    Ok(components.join("/"))
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
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
