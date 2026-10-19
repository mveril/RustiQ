use std::{env, error::Error, path::PathBuf};

use vergen_gix::{Cargo, Emitter, Rustc};

#[path = "build/provenance.rs"]
mod provenance;

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    emit_build_info(&manifest_dir)?;
    let source_dir = manifest_dir.join("assets");
    let target_dir = PathBuf::from(env::var("OUT_DIR")?).join("bat-assets");

    println!(
        "cargo:rerun-if-changed={}",
        source_dir.join("syntaxes").display()
    );

    bat::assets::build(
        &source_dir,
        true,
        false,
        &target_dir,
        env!("CARGO_PKG_VERSION"),
    )?;

    Ok(())
}

fn emit_build_info(manifest_dir: &std::path::Path) -> Result<(), Box<dyn Error>> {
    for name in ["VERGEN_GIT_SHA", "VERGEN_GIT_DIRTY"] {
        provenance::validate_vergen_git_override(name, &env::var(name).unwrap_or_default())?;
    }
    let cargo = Cargo::builder()
        .features(true)
        .opt_level(true)
        .target_triple(true)
        .build();
    let rustc = Rustc::builder().semver(true).llvm_version(true).build();
    Emitter::default()
        .add_instructions(&cargo)?
        .add_instructions(&rustc)?
        .emit()?;

    provenance::git_instructions(manifest_dir)?.emit()?;

    provenance::emit_fingerprints(manifest_dir)?;

    for name in [
        "RUSTIQ_BUILD_GIT_COMMIT",
        "RUSTIQ_BUILD_GIT_STATE",
        "RUSTIQ_BUILD_PROFILE",
        "RUSTIQ_BUILD_TARGET_CPU",
        "RUSTIQ_BUILD_SYSTEM",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
        let value = env::var(name).unwrap_or_default();
        provenance::validate_override(name, &value)?;
        println!("cargo:rustc-env={name}={value}");
    }

    emit_declared_profile_settings(manifest_dir)?;

    let target_features = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
    println!("cargo:rustc-env=RUSTIQ_TARGET_FEATURES={target_features}");
    let flags = env::var("CARGO_ENCODED_RUSTFLAGS")
        .or_else(|_| env::var("RUSTFLAGS"))
        .unwrap_or_default();
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
    println!("cargo:rerun-if-env-changed=RUSTFLAGS");
    let target_cpu = provenance::target_cpu_from_flags(&flags);
    let has_cpu_override = env::var("RUSTIQ_BUILD_TARGET_CPU").is_ok_and(|value| !value.is_empty());
    if !has_cpu_override && !target_cpu.is_empty() {
        println!("cargo:rustc-env=RUSTIQ_BUILD_TARGET_CPU={target_cpu}");
    }
    println!(
        "cargo:rustc-env=RUSTIQ_RUSTFLAGS_OBSERVED={}",
        !flags.is_empty()
    );

    Ok(())
}

fn emit_declared_profile_settings(manifest_dir: &std::path::Path) -> Result<(), Box<dyn Error>> {
    use std::str::FromStr;

    let profile = env::var("RUSTIQ_BUILD_PROFILE").unwrap_or_default();
    let manifest = fs_err_read_to_string(&manifest_dir.join("Cargo.toml"))?;
    let document = toml_edit::DocumentMut::from_str(&manifest)?;
    let profiles = document.get("profile").and_then(toml_edit::Item::as_table);
    for (key, env_name) in [
        ("lto", "RUSTIQ_LTO_DECLARED"),
        ("codegen-units", "RUSTIQ_CODEGEN_UNITS_DECLARED"),
        ("debug", "RUSTIQ_DEBUG_INFO_DECLARED"),
        ("overflow-checks", "RUSTIQ_OVERFLOW_CHECKS_DECLARED"),
        ("debug-assertions", "RUSTIQ_DEBUG_ASSERTIONS_DECLARED"),
    ] {
        let value = inherited_profile_value(profiles, &profile, key, 0).unwrap_or_default();
        println!("cargo:rustc-env={env_name}={value}");
    }
    Ok(())
}

fn inherited_profile_value(
    profiles: Option<&toml_edit::Table>,
    profile: &str,
    key: &str,
    depth: usize,
) -> Option<String> {
    if profile.is_empty() || depth > 8 {
        return None;
    }
    let table = profiles?.get(profile)?.as_table()?;
    if let Some(value) = table.get(key) {
        if let Some(string) = value.as_str() {
            return Some(string.to_owned());
        }
        if let Some(boolean) = value.as_bool() {
            return Some(boolean.to_string());
        }
        if let Some(integer) = value.as_integer() {
            return Some(integer.to_string());
        }
        return Some(value.to_string().trim().to_owned());
    }
    let inherited = table.get("inherits")?.as_str()?;
    inherited_profile_value(profiles, inherited, key, depth + 1)
}

fn fs_err_read_to_string(path: &std::path::Path) -> Result<String, Box<dyn Error>> {
    Ok(std::fs::read_to_string(path)?)
}
