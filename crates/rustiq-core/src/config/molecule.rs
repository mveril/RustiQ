use std::num::NonZeroU8;

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use super::Located;
use crate::molecules::{
    geometry::Geometry,
    molecule::{Molecule, MoleculeError},
    units::Units,
};

/// Error while applying molecular configuration to a geometry.
///
/// The optional spans identify values in an input frontend, without retaining
/// its source text or depending on calculation orchestration.
#[derive(Debug, Error, Diagnostic)]
#[error("{error}")]
pub struct MoleculeConfigError {
    #[source]
    pub error: MoleculeError,
    #[label("molecular charge")]
    pub charge_span: Option<SourceSpan>,
    #[label("spin multiplicity")]
    pub multiplicity_span: Option<SourceSpan>,
}

/// Molecular state and coordinate units, independent of geometry file paths.
#[derive(Debug, Clone, Copy)]
pub struct MoleculeConfig {
    pub units: Units,
    pub charge: Located<i32>,
    pub multiplicity: Located<NonZeroU8>,
}

impl Default for MoleculeConfig {
    fn default() -> Self {
        Self {
            units: Units::Bohr,
            charge: 0.into(),
            multiplicity: NonZeroU8::MIN.into(),
        }
    }
}

impl MoleculeConfig {
    pub fn build(&self, geometry: Geometry) -> Result<Molecule, MoleculeConfigError> {
        Molecule::try_new(
            geometry,
            self.units,
            self.charge.value,
            self.multiplicity.value,
        )
        .map_err(|error| MoleculeConfigError {
            charge_span: self.charge.span,
            multiplicity_span: matches!(error, MoleculeError::IncompatibleMultiplicity { .. })
                .then_some(self.multiplicity.span)
                .flatten(),
            error,
        })
    }
}
