use clap::Args;

/// Select an entry from a portable RustiQ bundle.
#[derive(Clone, Copy, Debug, Default, Args)]
pub(crate) struct BundleCalculationSelection {
    /// Select a zero-based calculation in the bundle.
    #[arg(long)]
    pub(crate) calculation: Option<usize>,
}

impl BundleCalculationSelection {
    /// Resolve an explicit index or the only calculation in a single-entry bundle.
    pub(crate) fn single(self, count: usize) -> miette::Result<usize> {
        match self.calculation {
            Some(index) if index < count => Ok(index),
            Some(index) => Err(miette::miette!(
                "Calculation index {index} is out of range. Use a zero-based index from 0 to {}.",
                count.saturating_sub(1)
            )),
            None if count == 1 => Ok(0),
            None => Err(miette::miette!(
                "This bundle contains {count} calculations. Use --calculation with a zero-based index."
            )),
        }
    }
}
