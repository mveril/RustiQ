use std::fs::File;
use std::path::PathBuf;

use rustiq_core::{basis::BasisFile, molecules::geometry::Geometry};

pub fn h2_inputs() -> Result<(Geometry, BasisFile), Box<dyn std::error::Error>> {
    let geometry_path = workspace_path("samples/h2/molecule.xyz")?.canonicalize()?;
    let basis_path = workspace_path("tests/data/sto-3g.json")?.canonicalize()?;
    let geometry = Geometry::from_file(File::open(geometry_path)?)?;
    let basis = BasisFile::from_reader(File::open(basis_path)?)?;
    Ok((geometry, basis))
}

pub fn workspace_path(path: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    Ok(root.join(path))
}

#[allow(
    dead_code,
    reason = "Retained helper supports scientific tests and benchmarks"
)]
pub fn data_path(filename: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/data");
    std::fs::create_dir_all(&directory)?;
    Ok(directory.canonicalize()?.join(filename))
}
