use toml_spanner::Toml;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy, Toml)]
#[toml(Toml)]
pub enum Units {
    Bohr,
    Angstrom,
}
