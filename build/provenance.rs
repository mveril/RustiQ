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
    Ok(())
}
