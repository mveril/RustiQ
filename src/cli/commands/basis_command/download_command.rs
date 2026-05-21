use std::cell::OnceCell;

use indicatif::{ProgressBar, ProgressStyle};

use crate::{basis::basis_store::BasisStore, cli::commands::AsyncRunnable};

#[derive(clap::Args, Debug)]
pub struct DownloadCommand {
    /// Name of the basis set
    pub name: String,
}

impl AsyncRunnable for DownloadCommand {
    async fn run_async(&self) {
        let store = BasisStore::default();
        let mut pb_cell = OnceCell::new(); // ProgressBar est stockée ici, elle est initialisée une seule fois.

        // Callback pour gérer la barre de progression
        let mut callback = |current: u64, total: Option<u64>| {
            // Initialisation de la barre de progression si elle n'existe pas encore
            if pb_cell.get().is_none() {
                if let Some(total_size) = total {
                    let progress_bar = ProgressBar::new(total_size).with_style(
                        ProgressStyle::with_template("{wide_bar:.cyan/blue} {percent}%")
                            .expect("Erreur de template invalide")
                            .progress_chars("█▓▒░"),
                    );
                    pb_cell
                        .set(progress_bar)
                        .expect("Failed to set progress bar");
                }
            }
            if let Some(pb) = pb_cell.get_mut() {
                pb.set_position(current);
            }
        };
        store.download(&self.name, &mut callback).await.unwrap();
        if let Some(pb) = pb_cell.get_mut() {
            pb.finish_with_message(format!("Basis {} downloaded.", self.name));
        } else {
            print!("Basis {} downloaded.", self.name);
        }
    }
}
