use std::{num::NonZero, ops::Deref};

use thiserror::Error;

use super::{convert_length::convert_length, geometry::Geometry, units::Units};

pub struct Molecule {
    pub geometry: Geometry,
    pub unit: Units,
    pub charge: i32,
    pub multiplicity: NonZero<u8>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MoleculeError {
    #[error("invalid molecular charge: nuclear charge = {nuclear_charge}, molecular charge = {charge}, total electrons = {electrons}")]
    InvalidCharge {
        nuclear_charge: i32,
        charge: i32,
        electrons: i32,
    },
}

impl Deref for Molecule {
    type Target = Geometry;

    fn deref(&self) -> &Self::Target {
        &self.geometry
    }
}

impl From<Geometry> for Molecule {
    fn from(geometry: Geometry) -> Self {
        // SAFETY: A neutral molecule cannot have a negative electron count.
        unsafe { Molecule::new_unchecked(geometry, Units::Bohr, 0, NonZero::new_unchecked(1)) }
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
        molecule.validate_charge()?;
        Ok(molecule)
    }

    /// Builds a molecule without validating the charge-derived electron count.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `nuclear_charge - charge >= 0`.
    pub unsafe fn new_unchecked(
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

    fn validate_charge(&self) -> Result<(), MoleculeError> {
        let nuclear_charge = self.nuclear_charge();
        let electrons = nuclear_charge - self.charge;
        if electrons < 0 {
            return Err(MoleculeError::InvalidCharge {
                nuclear_charge,
                charge: self.charge,
                electrons,
            });
        }
        Ok(())
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
    fn try_new_rejects_charge_larger_than_nuclear_charge() {
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
            MoleculeError::InvalidCharge {
                nuclear_charge: 1,
                charge: 2,
                electrons: -1
            }
        );
    }
}
