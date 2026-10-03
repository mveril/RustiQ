use toml_spanner::Toml;

#[derive(Debug, Toml)]
#[toml(Toml)]
pub struct BasisConfig {
    pub name: String,
}
