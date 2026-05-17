use serde::{Deserialize, Serialize};

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Units {
    Bohr,
    Angstrom,
}
