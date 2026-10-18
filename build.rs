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
    let cargo = Cargo::builder()
        .debug(true)
        .features(true)
        .opt_level(true)
        .target_triple(true)
        .build();
    let rustc = Rustc::builder().semver(true).commit_hash(true).build();
    Emitter::default()
        .add_instructions(&cargo)?
        .add_instructions(&rustc)?
        .emit()?;

    provenance::git_instructions(manifest_dir)?.emit()?;

    for name in ["RUSTIQ_BUILD_GIT_COMMIT", "RUSTIQ_BUILD_GIT_STATE"] {
        println!("cargo:rerun-if-env-changed={name}");
        let value = env::var(name).unwrap_or_default();
        provenance::validate_override(name, &value)?;
        println!("cargo:rustc-env={name}={value}");
    }

    // Git HEAD watches miss unstaged edits and reversions. A deliberately absent
    // file makes Cargo refresh provenance on every build, including worktrees.
    println!(
        "cargo:rerun-if-changed={}",
        PathBuf::from(env::var("OUT_DIR")?)
            .join("refresh-provenance")
            .display()
    );
    Ok(())
}
