use super::units::Units;
use physical_constants::ATOMIC_UNIT_OF_LENGTH;
use std::ops::{Div, Mul};

const CONV_FACTOR: f64 = ATOMIC_UNIT_OF_LENGTH * 1e10;

pub fn convert_length<T>(value: T, from_unit: Units, to_unit: Units) -> T
where
    T: Mul<f64, Output = T> + Div<f64, Output = T> + Copy,
{
    match (from_unit, to_unit) {
        (Units::Bohr, Units::Angstrom) => value * CONV_FACTOR,
        (Units::Angstrom, Units::Bohr) => value / CONV_FACTOR,
        _ => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*; // Cela importe la fonction `convert_length` et les types associés.

    // Test de conversion de Bohr vers Angstrom
    #[test]
    fn test_conversion_bohr_to_angstrom() {
        let bohr_length = 1.0; // 1 Bohr
        let angstrom_length = convert_length(bohr_length, Units::Bohr, Units::Angstrom);
        // 1 Bohr devrait être approximativement égal à 0.529177 Å.
        assert!(
            (angstrom_length - 0.52917721092).abs() < 1e-10,
            "Bohr to Angstrom conversion failed"
        );
    }

    // Test de conversion d'Angstrom vers Bohr
    #[test]
    fn test_conversion_angstrom_to_bohr() {
        let angstrom_length = 0.52917721092; // 0.529177 Å
        let bohr_length = convert_length(angstrom_length, Units::Angstrom, Units::Bohr);
        // 0.529177 Å devrait être équivalent à 1 Bohr.
        assert!(
            (bohr_length - 1.0).abs() < 1e-10,
            "Angstrom to Bohr conversion failed"
        );
    }

    // Test pour les cas où l'unité source et destination sont identiques
    #[test]
    fn test_no_conversion() {
        let value = 5.0; // Testons une longueur arbitraire de 5 unités
                         // Conversion sans changement d'unité (Bohr -> Bohr)
        let bohr_result = convert_length(value, Units::Bohr, Units::Bohr);
        assert_eq!(bohr_result, value, "No conversion (Bohr to Bohr) failed");

        // Conversion sans changement d'unité (Angstrom -> Angstrom)
        let angstrom_result = convert_length(value, Units::Angstrom, Units::Angstrom);
        assert_eq!(
            angstrom_result, value,
            "No conversion (Angstrom to Angstrom) failed"
        );
    }
}
