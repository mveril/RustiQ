use std::fs::File;

use rayon::iter::{ParallelBridge, ParallelIterator};
use tabled::Table;

use crate::{
    basis::{basis_store::BasisStore, basisfile::BasisFile},
    cli::{
        commands::{CommandResult, Runnable},
        ux::{bat, BasisTableItem},
    },
};

#[derive(clap::Args, Debug)]
pub struct ListCommand {
    /// Check basis sets available online
    #[cfg(feature = "online")]
    #[arg(long)]
    pub online: bool,
    /// Increase the verbosity
    #[arg(short, long)]
    pub verbose: bool,
}

impl Runnable for ListCommand {
    fn run(&self) -> CommandResult {
        let store = BasisStore::default();
        #[cfg(feature = "online")]
        if self.online {
            let list = store.list_online_sync()?;
            if self.verbose {
                let items = list.into_values().map(BasisTableItem::from);
                bat::print_paged(&Table::new(items).to_string());
            } else {
                let mut str = String::new();
                for item in list.keys() {
                    str.push_str(item);
                    str.push('\n');
                }
                bat::print_paged(&str);
            }
            return Ok(());
        }

        let list = store.list()?;
        if self.verbose {
            let v: anyhow::Result<Vec<_>> = list
                .par_bridge()
                .map(|item| {
                    let item = item?;
                    let file_content =
                        serde_json::from_reader::<_, BasisFile>(File::open(item.path())?)?;
                    Ok(BasisTableItem::from(file_content))
                })
                .collect();
            bat::print_paged(&Table::new(v?).to_string())
        } else {
            let mut str = String::new();
            for item in list {
                match item {
                    Ok(entry) => {
                        if let Some(name) = entry.path().file_stem() {
                            str.push_str(&name.to_string_lossy());
                            str.push('\n');
                        }
                    }
                    Err(err) => eprint!("Failed to load an item: {err}."),
                }
            }
            bat::print_paged(&str);
        }
        Ok(())
    }
}
