use std::{fs::File, path::PathBuf};

use crate::{
    basis::basis_store::BasisStore,
    cli::commands::{CommandResult, Runnable},
};

#[derive(clap::Args, Debug)]
pub struct ImportCommand {
    /// Path to a basis set JSON file
    pub path: PathBuf,
}

impl Runnable for ImportCommand {
    fn run(&self) -> CommandResult {
        let store = BasisStore::default();
        let file = File::open(&self.path)?;
        let name = store.import(file)?;
        print!("Basis {} imported.", name);
        Ok(())
    }
}
