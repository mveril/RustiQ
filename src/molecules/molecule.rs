use std::{num::NonZero, ops::Deref};

use thiserror::Error;

use super::{convert_length::convert_length, geometry::Geometry, units::Units};

pub struct Molecule {
    geometry: Geometry,
    unit: Units,
    charge: i32,
    multiplicity: NonZero<u8>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MoleculeError {
    #[error("invalid molecular electron count: nuclear charge = {nuclear_charge}, molecular charge = {charge}, total electrons = {electrons}")]
    InvalidElectronCount {
        nuclear_charge: i32,
        charge: i32,
        electrons: i32,
    },
    #[error("invalid electron configuration: total electrons = {electrons}, multiplicity = {multiplicity}")]
    IncompatibleMultiplicity { electrons: usize, multiplicity: u8 },
}

impl Deref for Molecule {
    type Target = Geometry;

    fn deref(&self) -> &Self::Target {
        &self.geometry
    }
}

impl From<Geometry> for Molecule {
    fn from(geometry: Geometry) -> Self {
        Self::try_new(geometry, Units::Bohr, 0, std::num::NonZeroU8::MIN)
            .expect("a neutral geometry must have a closed-shell singlet electron configuration")
    }
}

impl Molecule {
    pub fn try_new(
        geometry: Geometry,
        unit: Units,
        charge: i32,
        multiplicity: NonZero<u8>,
    ) -> Result<Self, MoleculeError> {
        let molecule = Self {
            geometry,
            unit,
            charge,
            multiplicity,
        };
        molecule.validate_electron_configuration()?;
        Ok(molecule)
    }

    /// Builds a molecule without validating its electron configuration.
    ///
    /// This is restricted to the crate for tests and transitional internal code.
    #[allow(dead_code)]
    pub(crate) fn new_unchecked(
        geometry: Geometry,
        unit: Units,
        charge: i32,
        multiplicity: NonZero<u8>,
    ) -> Self {
        Self {
            geometry,
            unit,
            charge,
            multiplicity,
        }
    }

    fn validate_electron_configuration(&self) -> Result<(), MoleculeError> {
        let nuclear_charge = self.nuclear_charge();
        let electrons = nuclear_charge - self.charge;
        if electrons <= 0 {
            return Err(MoleculeError::InvalidElectronCount {
                nuclear_charge,
                charge: self.charge,
                electrons,
            });
        }
        let unpaired_electrons = self.unpaired_electrons() as i32;
        if unpaired_electrons > electrons || (electrons - unpaired_electrons) % 2 != 0 {
            return Err(MoleculeError::IncompatibleMultiplicity {
                electrons: electrons as usize,
                multiplicity: self.multiplicity.get(),
            });
        }
        Ok(())
    }

    pub fn geometry(&self) -> &Geometry {
        &self.geometry
    }

    #[allow(dead_code)]
    pub fn unit(&self) -> Units {
        self.unit
    }

    #[allow(dead_code)]
    pub fn charge(&self) -> i32 {
        self.charge
    }

    pub fn multiplicity(&self) -> NonZero<u8> {
        self.multiplicity
    }

    pub fn convert_to(&mut self, unit: Units) {
        if self.unit == unit {
            return;
        }

        for atom in &mut self.geometry.atoms {
            atom.position = convert_length(atom.position, self.unit, unit);
        }
        self.unit = unit;
    }

    fn nuclear_charge(&self) -> i32 {
        self.atoms
            .iter()
            .map(|a| a.element.atomic_number)
            .sum::<u32>() as i32
    }

    pub fn total_electrons(&self) -> usize {
        (self.nuclear_charge() - self.charge) as usize
    }

    pub fn unpaired_electrons(&self) -> u8 {
        self.multiplicity.get() - 1
    }

    pub fn occupied_orbitals(&self) -> usize {
        ((self.total_electrons() - self.unpaired_electrons() as usize) / 2)
            + self.unpaired_electrons() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::molecules::{atom::Atom, geometry::Geometry};
    use nalgebra::point;
    use std::num::NonZeroU8;

    fn atoms(symbols: &[&str]) -> Vec<Atom> {
        let elements = periodic_table::periodic_table();
        symbols
            .iter()
            .enumerate()
            .map(|(index, symbol)| {
                let element = elements
                    .iter()
                    .find(|element| element.symbol == *symbol)
                    .unwrap();
                Atom::new(element, point![0.0, 0.0, index as f64])
            })
            .collect()
    }

    #[test]
    fn total_electrons_supports_large_neutral_molecules() {
        let mut symbols = Vec::new();
        symbols.extend(std::iter::repeat_n("C", 27));
        symbols.extend(std::iter::repeat_n("H", 46));
        symbols.push("O");

        let molecule = Molecule::try_new(
            Geometry::new("cholesterol".to_string(), atoms(&symbols)),
            Units::Bohr,
            0,
            NonZeroU8::new(1).unwrap(),
        )
        .unwrap();

        assert_eq!(molecule.total_electrons(), 216);
    }

    #[test]
    fn try_new_rejects_non_positive_electron_count() {
        let error = match Molecule::try_new(
            Geometry::new("overcharged hydrogen".to_string(), atoms(&["H"])),
            Units::Bohr,
            2,
            NonZeroU8::new(1).unwrap(),
        ) {
            Ok(_) => panic!("expected invalid molecular charge"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            MoleculeError::InvalidElectronCount {
                nuclear_charge: 1,
                charge: 2,
                electrons: -1
            }
        );
    }

    #[test]
    fn try_new_rejects_incompatible_multiplicity() {
        let result = Molecule::try_new(
            Geometry::new("triplet hydrogen".to_string(), atoms(&["H"])),
            Units::Bohr,
            0,
            NonZeroU8::new(3).unwrap(),
        );

        assert!(matches!(
            result,
            Err(MoleculeError::IncompatibleMultiplicity {
                electrons: 1,
                multiplicity: 3,
            })
        ));
    }

    #[test]
    fn try_new_rejects_multiplicity_with_wrong_parity() {
        let result = Molecule::try_new(
            Geometry::new("doublet H2".to_string(), atoms(&["H", "H"])),
            Units::Bohr,
            0,
            NonZeroU8::new(2).unwrap(),
        );

        assert!(matches!(
            result,
            Err(MoleculeError::IncompatibleMultiplicity {
                electrons: 2,
                multiplicity: 2,
            })
        ));
    }
}
