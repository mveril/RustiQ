use bat::PrettyPrinter;
use miette::IntoDiagnostic;
use rayon::iter::{ParallelBridge, ParallelIterator};
use tabled::Table;

use crate::{
    basis::BasisStore,
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
                for item in list.keys() {
                    str.push_str(item);
                    str.push('\n');
                }
                pagin_print(&str);
            }
            return Ok(());
        }

        let list = store.list().into_diagnostic()?;
        if self.verbose {
            let v: std::io::Result<Vec<_>> =
                list.par_bridge()
                    .map(|item| {
                        let item = item?;
                        let path = item.path();
                        let name = path
                            .file_stem()
                            .and_then(|name| name.to_str())
                            .ok_or_else(|| std::io::Error::other("invalid basis file name"))?;
                        let basis_file = store
                            .get(name)
                            .map_err(std::io::Error::other)?
                            .ok_or_else(|| {
                                std::io::Error::new(
                                    std::io::ErrorKind::NotFound,
                                    format!("basis file '{name}' disappeared during listing"),
                                )
                            })?;
                        Ok(BasisTableItem::from(basis_file))
                    })
                    .collect();
            pagin_print(&Table::new(v.into_diagnostic()?).to_string())
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
            pagin_print(&str);
        }
        Ok(())
    }
}
