use crate::{
    basis::basis_store::BasisStore,
    cli::commands::{CommandResult, Runnable},
};
use clap::{ArgAction, ArgGroup};
use miette::IntoDiagnostic;
#[derive(clap::Args, Debug)]
#[command(group(
    ArgGroup::new("target")
        .required(true)
        .multiple(false)
        .args(["names", "all"])
))]
pub struct RemoveCommand {
    /// Names of the basis sets; this argument can be used multiple times
    #[arg(value_name = "NAME")]
    pub names: Vec<String>,
    #[arg(
        long,
        short,
        action = ArgAction::SetTrue
    )]
    /// Remove all basis set installed
    pub all: bool,
}

impl Runnable for RemoveCommand {
    fn run(&self) -> CommandResult {
        let store = BasisStore::default();
        if self.all {
            store.remove_all()
        } else {
            store.remove(&self.names)
        }
        .into_diagnostic()
    }
}
