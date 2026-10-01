use bat::PrettyPrinter;
use miette::IntoDiagnostic;
use rayon::iter::{ParallelBridge, ParallelIterator};
use tabled::{
    settings::{object::Rows, Color, Modify},
    Table,
};

use crate::cli::{
    commands::{CommandResult, Runnable},
    ux::BasisTableItem,
};
use rustiq_core::basis::BasisEntry;

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
        let store = crate::cli::directories::basis_store();
        #[cfg(feature = "online")]
        if self.online {
            let list = store.list_online_sync().into_diagnostic()?;
            if self.verbose {
                let items = list.into_values().map(BasisTableItem::from);
                pagin_print(&render_table(items));
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

        let list = store.list().into_diagnostic()?;
        if self.verbose {
            let v: Result<Vec<_>, rustiq_core::basis::FileError> = list
                .par_bridge()
                .map(|item| {
                    item.map(BasisEntry::into_basis_file)
                        .map(BasisTableItem::from)
                })
                .collect();
            pagin_print(&render_table(v.into_diagnostic()?))
        } else {
            let mut str = String::new();
            for item in list {
                match item {
                    Ok(entry) => {
                        str.push_str(entry.name());
                        str.push('\n');
                    }
                    Err(err) => eprint!("Failed to load an item: {err}."),
                }
            }
            pagin_print(&str);
        }
        Ok(())
    }
}

fn render_table(items: impl IntoIterator<Item = BasisTableItem>) -> String {
    let mut table = Table::new(items);
    if crate::cli::color::enabled() {
        table.with(Modify::new(Rows::first()).with(Color::FG_CYAN | Color::BOLD));
    }
    table.to_string()
}
