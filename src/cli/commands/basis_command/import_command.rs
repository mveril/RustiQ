use std::{fs::File, path::PathBuf};

use crate::{basis::basis_store::BasisStore, cli::commands::Runnable};

#[derive(clap::Args, Debug)]
pub struct ImportCommand {
    /// Path to a basis set JSON file
    pub path: PathBuf,
}

impl Runnable for ImportCommand {
    fn run(&self) {
        let store = BasisStore::default();
        let file = File::open(&self.path).unwrap();
        let name = store.import(file).unwrap();
        print!("Basis {} imported.", name);
    }
}
