use std::num::NonZeroU8;

use super::Located;
use crate::{
    calculation::CalculationError,
    molecules::{
        geometry::Geometry,
        molecule::{Molecule, MoleculeError},
        units::Units,
    },
};

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
    pub fn build(&self, geometry: Geometry) -> Result<Molecule, CalculationError> {
        Molecule::try_new(
            geometry,
            self.units,
            self.charge.value,
            self.multiplicity.value,
        )
        .map_err(|error| CalculationError::Molecule {
            charge_span: self.charge.span,
            multiplicity_span: matches!(error, MoleculeError::IncompatibleMultiplicity { .. })
                .then_some(self.multiplicity.span)
                .flatten(),
            error,
        })
    }
}
