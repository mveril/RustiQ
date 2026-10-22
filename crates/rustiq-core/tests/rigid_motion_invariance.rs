//! Rigid motions preserve scalar observables, including with oriented p orbitals.
use std::{f64::consts::PI, num::NonZeroU8, sync::LazyLock};

use nalgebra::{Rotation3, Vector3};
use proptest::{prelude::*, test_runner::TestCaseResult};
use rustiq_core::{
    basis::BasisFile,
    calculation::{CalculationBuilder, CalculationExecution, CalculationResult, HfOutcome},
    config::{HfConfig, HfMethod, MoleculeConfig, Mp2Config},
    molecules::{geometry::Geometry, units::Units},
};

// Absolute tolerance in Hartree for energies, dimensionless for spin.
// The original seeded sample passed at 1e-12 and failed at 1e-13.
const ABSOLUTE_TOLERANCE: f64 = 1e-12;

fn calculate(
    geometry: &Geometry,
    method: HfMethod,
    multiplicity: u8,
) -> Result<CalculationResult, TestCaseError> {
    let basis = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..])
        .expect("STO-3G fixture must be valid");
    let result = CalculationBuilder::new(geometry, &basis)
        .with_molecule_config(MoleculeConfig {
            units: Units::Angstrom,
            multiplicity: NonZeroU8::new(multiplicity)
                .expect("fixture multiplicity must be positive")
                .into(),
            ..Default::default()
        })
        .with_hf(HfConfig {
            method: method.into(),
            convergence_threshold: 1e-10.try_into().expect("positive finite SCF threshold"),
            ..Default::default()
        })
        .with_mp2(Mp2Config::default())
        .execute()
        .map_err(|error| TestCaseError::fail(format!("calculation failed: {error}")))?;
    prop_assert!(matches!(result.hf, HfOutcome::Converged(_)));
    prop_assert!(result.mp2.is_some());
    Ok(result)
}

fn observables(result: &CalculationResult) -> Vec<(&'static str, f64)> {
    let scf = &result.hf.summary().scf;
    let mp2 = result.mp2.as_ref().expect("calculation must include MP2");
    let mut values = vec![
        ("nuclear repulsion", scf.nuclear_repulsion_energy),
        ("HF electronic energy", scf.electronic_energy),
        ("HF total energy", scf.total_energy),
        ("kinetic energy", scf.energy_details.kinetic_energy),
        (
            "nuclear attraction",
            scf.energy_details.nuclear_attraction_energy,
        ),
        (
            "electron repulsion",
            scf.energy_details.electron_repulsion_energy,
        ),
        ("MP2 correlation energy", mp2.correlation_energy),
        ("MP2 electronic energy", mp2.electronic_energy),
        (
            "MP2 total energy",
            mp2.electronic_energy + scf.nuclear_repulsion_energy,
        ),
    ];
    if let Some(spin) = scf.spin {
        values.extend([
            ("spin squared", spin.s_squared),
            ("spin contamination", spin.spin_contamination),
        ]);
    }
    values
}

struct ReferenceCase {
    name: &'static str,
    geometry: Geometry,
    method: HfMethod,
    multiplicity: u8,
    observables: Vec<(&'static str, f64)>,
}

impl ReferenceCase {
    fn new(name: &'static str, xyz: &[u8], method: HfMethod, multiplicity: u8) -> Self {
        let geometry = Geometry::from_reader(xyz).expect("geometry fixture must be valid");
        let observables = observables(
            &calculate(&geometry, method, multiplicity)
                .expect("reference calculation must converge"),
        );
        Self {
            name,
            geometry,
            method,
            multiplicity,
            observables,
        }
    }
}

// Cache only immutable fixture data and scalar references. Every generated case
// builds a fresh calculation, independently of execution and shrinking order.
static REFERENCES: LazyLock<[ReferenceCase; 2]> = LazyLock::new(|| {
    [
        ReferenceCase::new(
            "H2O/RHF",
            include_bytes!("data/samples/h2o/h2o.xyz"),
            HfMethod::Rhf,
            1,
        ),
        ReferenceCase::new(
            "OH/UHF",
            include_bytes!("data/samples/oh/oh.xyz"),
            HfMethod::Uhf,
            2,
        ),
    ]
});

fn translations() -> impl Strategy<Value = Vector3<f64>> {
    // Moderate displacements in Angstrom avoid loss of coordinate precision.
    prop::array::uniform3(-5.0_f64..5.0).prop_map(Vector3::from)
}

fn rotations() -> impl Strategy<Value = Rotation3<f64>> {
    // Euler angles cover general proper rotations. Shrinking toward zero keeps
    // rotations valid and tends toward the identity; Haar sampling is not needed.
    prop::array::uniform3(-PI..PI)
        .prop_map(|angles| Rotation3::from_euler_angles(angles[0], angles[1], angles[2]))
}

fn check_rigid_motion(translation: Vector3<f64>, rotation: Rotation3<f64>) -> TestCaseResult {
    // Exercise both molecular references for every generated transformation.
    for reference in REFERENCES.iter() {
        let mut geometry = reference.geometry.clone();
        for atom in &mut geometry.atoms {
            atom.position = rotation * atom.position + translation;
        }
        let result = calculate(&geometry, reference.method, reference.multiplicity)?;
        let actual = observables(&result);
        prop_assert_eq!(actual.len(), reference.observables.len());
        for ((name, actual), (expected_name, expected)) in actual.iter().zip(&reference.observables)
        {
            prop_assert_eq!(name, expected_name);
            let difference = (actual - expected).abs();
            prop_assert!(difference < ABSOLUTE_TOLERANCE,
                "{}: {name}: actual={actual:e}, expected={expected:e}, absolute difference={difference:e}, tolerance={ABSOLUTE_TOLERANCE:e}", reference.name);
        }
    }
    Ok(())
}

fn property_config() -> ProptestConfig {
    let mut config = ProptestConfig::default();
    // Each case runs RHF/MP2 and UHF/MP2. Keep ordinary runs small, while allowing
    // extended runs with PROPTEST_CASES and reproduction with PROPTEST_RNG_SEED.
    if std::env::var_os("PROPTEST_CASES").is_none() {
        config.cases = 16;
    }
    config
}

proptest! {
    #![proptest_config(property_config())]

    #[test]
    fn scalar_observables_are_translation_invariant(translation in translations()) {
        check_rigid_motion(translation, Rotation3::identity())?;
    }

    #[test]
    fn scalar_observables_are_rotation_invariant(rotation in rotations()) {
        check_rigid_motion(Vector3::zeros(), rotation)?;
    }

    #[test]
    fn scalar_observables_are_rigid_motion_invariant(
        translation in translations(),
        rotation in rotations(),
    ) {
        check_rigid_motion(translation, rotation)?;
    }
}
