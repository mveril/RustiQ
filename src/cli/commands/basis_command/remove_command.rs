use crate::{
    basis::basis_store::BasisStore,
    cli::commands::{CommandResult, Runnable},
};

#[derive(clap::Args)]
pub struct DownloadCommand {
    /// Name of the basis set
    pub name: String,
}

impl Runnable for DownloadCommand {
    fn run(&self) -> CommandResult {
        println!("Downloading basis set: {}", self.name);
        // Logique de téléchargement ici
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct RemoveCommand {
    /// Names of the basis sets; this argument can be used multiple times
    #[arg(short, long, value_name = "NAME")]
    pub names: Vec<String>,
}

impl Runnable for RemoveCommand {
    fn run(&self) -> CommandResult {
        let store = BasisStore::default();
        if self.names.is_empty() {
            store.remove_all()
        } else {
            store.remove(&self.names)
        }?;
        Ok(())
    }
}
