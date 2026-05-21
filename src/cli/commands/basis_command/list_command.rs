use std::fs::File;

use rayon::iter::{ParallelBridge, ParallelIterator};
use tabled::Table;

use crate::{
    basis::{basis_store::BasisStore, basisfile::BasisFile},
    cli::{commands::Runnable, ux::BasisTableItem},
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
    fn run(&self) {
        let store = BasisStore::default();
        #[cfg(feature = "online")]
        if self.online {
            let list = store.list_online_sync().unwrap();
            if self.verbose {
                let items = list.into_values().map(BasisTableItem::from);
                print!("{}", Table::new(items));
            } else {
                for name in list.keys() {
                    println!("{}", name);
                }
            }
            return;
        }

        match store.list() {
            Ok(list) => {
                if self.verbose {
                    let v: Vec<_> = list
                        .par_bridge()
                        .map(|item| {
                            let item = item.unwrap();
                            let file_content = serde_json::from_reader::<_, BasisFile>(
                                File::open(item.path()).unwrap(),
                            )
                            .unwrap();
                            BasisTableItem::from(file_content)
                        })
                        .collect();
                    print!("{}", Table::new(v))
                } else {
                    for item in list {
                        match item {
                            Ok(entry) => {
                                if let Some(name) = entry.path().file_stem() {
                                    println!("{}", name.to_string_lossy());
                                }
                            }
                            Err(err) => eprint!("Failed to load an item: {err}."),
                        }
                    }
                }
            }
            Err(err) => eprintln!("Failed to list {}", err),
        };
    }
}
