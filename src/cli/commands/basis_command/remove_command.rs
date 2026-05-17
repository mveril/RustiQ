use crate::{basis::basis_store::BasisStore, cli::commands::Runable};

#[derive(clap::Args)]
pub struct DownloadCommand {
    /// Name of the basis set
    pub name: String,
}

impl Runable for DownloadCommand {
    fn run(&self) {
        println!("Downloading basis set: {}", self.name);
        // Logique de téléchargement ici
    }
}

#[derive(clap::Args, Debug)]
pub struct RemoveCommand {
    /// Names of the basis sets; this argument can be used multiple times
    #[arg(short, long, value_name = "NAME")]
    pub names: Vec<String>,
}

impl Runable for RemoveCommand {
    fn run(&self) {
        let store = BasisStore::default();
        if self.names.is_empty() {
            store.remove_all()
        } else {
            store.remove(&self.names)
        }
        .unwrap_or_else(|err| println!("Failed to remove {}", err));
    }
}
