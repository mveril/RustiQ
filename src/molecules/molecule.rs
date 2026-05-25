use std::{num::NonZero, ops::Deref};

use super::{convert_length::convert_length, geometry::Geometry, units::Units};

pub struct Molecule {
    pub geometry: Geometry,
    pub unit: Units,
    pub charge: i8,
    pub multiplicity: NonZero<u8>,
}

impl Deref for Molecule {
    type Target = Geometry;

    fn deref(&self) -> &Self::Target {
        &self.geometry
    }
}

impl From<Geometry> for Molecule {
    fn from(geometry: Geometry) -> Self {
        Molecule {
            geometry,
            unit: Units::Bohr,
            charge: 0,
            multiplicity: unsafe { NonZero::new_unchecked(1) },
        }
    }
}

impl Molecule {
    pub fn new(geometry: Geometry, unit: Units, charge: i8, multiplicity: NonZero<u8>) -> Self {
        Self {
            geometry,
            unit,
            charge,
            multiplicity,
        }
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

    pub fn total_electrons(&self) -> i8 {
        let n_charge = self
            .atoms
            .iter()
            .map(|a| a.element.atomic_number)
            .sum::<u32>();
        (n_charge as i64 - self.charge as i64) as i8
    }

    pub fn unpaired_electrons(&self) -> u8 {
        self.multiplicity.get() - 1
    }

    pub fn occupied_orbitals(&self) -> usize {
        ((self.total_electrons() as usize - self.unpaired_electrons() as usize) / 2)
            + self.unpaired_electrons() as usize
    }
}
