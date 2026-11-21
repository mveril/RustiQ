use std::io::{self, Write};

use thiserror::Error;

use super::{ArtifactError, ConvergedHf, ConvergedHfArtifact, RustiQData};

mod molden;
use molden::MoldenBasis;

/// Format for exporting a saved calculation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ExportFormat {
    /// Resolved geometry, AO basis, and converged HF orbitals in Molden format.
    Molden,
}

/// Failure while preparing a saved calculation for export.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ExportError {
    /// The calculation has no resolved geometry and basis.
    #[error("the calculation has no resolved context")]
    MissingContext,
    /// The calculation has no saved converged HF orbitals.
    #[error("the calculation has no saved converged HF orbitals")]
    MissingOrbitals,
    /// The saved orbital artifact could not be read or validated.
    #[error("could not read the converged HF artifact: {0}")]
    Artifact(#[from] ArtifactError),
    /// Molden cannot represent the stored AO basis.
    #[error("unsupported Molden basis: {0}")]
    UnsupportedBasis(String),
}

/// A validated export ready to write to a caller-provided destination.
pub struct ArtifactExport {
    data: ExportData,
}

enum ExportData {
    Molden { basis: MoldenBasis, hf: ConvergedHf },
}

impl ArtifactExport {
    /// Writes the export to the destination and flushes it.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if writing or flushing the destination fails.
    pub fn write_to(&self, output: impl Write) -> io::Result<()> {
        match &self.data {
            ExportData::Molden { basis, hf } => basis.write(hf, output),
        }
    }
}

impl RustiQData {
    /// Prepares a format-specific export from this calculation's saved data.
    ///
    /// Molden uses the resolved geometry and AO basis and loads the saved converged
    /// HF orbitals. Stream the result with [`ArtifactExport::write_to`].
    ///
    /// # Errors
    ///
    /// Returns an error if the required saved data is missing or invalid, or if
    /// the requested format cannot represent the basis.
    pub fn export(&self, format: ExportFormat) -> Result<ArtifactExport, ExportError> {
        let data = match format {
            ExportFormat::Molden => {
                let context = self.calculation().ok_or(ExportError::MissingContext)?;
                let hf = self
                    .get::<ConvergedHfArtifact>()?
                    .ok_or(ExportError::MissingOrbitals)?;
                let basis = MoldenBasis::from_context(context)?;
                ExportData::Molden { basis, hf }
            }
        };
        Ok(ArtifactExport { data })
    }
}
