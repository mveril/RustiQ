use bat::PrettyPrinter;
use miette::IntoDiagnostic;
use rayon::iter::{ParallelBridge, ParallelIterator};
use tabled::Table;

use crate::{
    basis::{BasisFile, BasisStore},
    cli::{
        commands::{CommandResult, Runnable},
        ux::BasisTableItem,
    },
};

fn pagin_print(content: &str) {
    if PrettyPrinter::new()
        .colored_output(false)
        .strip_ansi(bat::StripAnsiMode::Never)
        .input_from_bytes(content.as_bytes())
        .paging_mode(bat::PagingMode::QuitIfOneScreen)
        .print()
        .is_err()
    {
        println!("{}", content)
    }
}

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
            let list = store.list_online_sync().into_diagnostic()?;
            if self.verbose {
                let items = list.into_values().map(BasisTableItem::from);
                pagin_print(&Table::new(items).to_string());
            } else {
                let mut str = String::new();
                for item in list.values() {
                    str.push_str(&item.display_name);
                    str.push('\n');
                }
                pagin_print(&str);
            }
            return Ok(());
        }

        let items: std::io::Result<Vec<_>> = store
            .list()
            .into_diagnostic()?
            .par_bridge()
            .map(|entry| {
                let file = std::fs::File::open(entry?.path())?;
                BasisFile::from_reader(file)
                    .map(BasisTableItem::from)
                    .map_err(std::io::Error::other)
            })
            .collect();
        let mut items = items.into_diagnostic()?;
        items.sort_by(|a, b| a.name.cmp(&b.name));
        if self.verbose {
            pagin_print(&Table::new(items).to_string());
        } else {
            let names = items.into_iter().map(|item| item.name).collect::<Vec<_>>();
            pagin_print(&names.join("\n"));
        }
        Ok(())
    }
}
