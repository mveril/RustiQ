use std::fs::File;

use crate::basis::basisfile::BasisFile;

pub(crate) fn load_minimal_basis_file() -> BasisFile {
    let file = File::open("tests/data/sto-3g.json").unwrap();
    serde_json::from_reader(file).unwrap()
}
