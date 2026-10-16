use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NuclearRepulsionError {
    #[error("nuclear repulsion is undefined because two atoms occupy the same position")]
    CoincidentAtoms,
}
