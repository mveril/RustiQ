use std::{env, error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
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
