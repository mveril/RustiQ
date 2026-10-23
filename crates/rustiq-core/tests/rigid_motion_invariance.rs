//! Rigid motions preserve scalar observables, including with oriented p orbitals.
use std::{
    f64::consts::{FRAC_PI_2, PI},
    num::NonZeroU8,
};

use nalgebra::{Isometry3, Rotation3, Vector3};
use proptest::{prelude::*, test_runner::TestCaseResult};
use rstest::{fixture, rstest};
use rustiq_core::{
    basis::BasisFile,
    calculation::{CalculationBuilder, CalculationExecution, CalculationResult, HfOutcome},
    config::{HfConfig, HfMethod, MoleculeConfig, Mp2Config},
    molecules::{geometry::Geometry, units::Units},
};

// Absolute tolerance in Hartree for energies, dimensionless for spin.
const ABSOLUTE_TOLERANCE: f64 = 1e-12;
// SCF termination checks energy change and residual, not observable error.
// Use a tighter residual threshold to reduce density-dependent variation.
const SCF_CONVERGENCE_THRESHOLD: f64 = 1e-12;

#[fixture]
#[once]
fn basis() -> BasisFile {
    BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..])
        .expect("STO-3G fixture must be valid")
}

#[fixture]
#[once]
fn h2o_geometry() -> Geometry {
    Geometry::from_reader(&include_bytes!("data/samples/h2o/h2o.xyz")[..])
        .expect("H2O geometry fixture must be valid")
}

#[fixture]
#[once]
fn oh_geometry() -> Geometry {
    Geometry::from_reader(&include_bytes!("data/samples/oh/oh.xyz")[..])
        .expect("OH geometry fixture must be valid")
}

fn calculate(
    geometry: &Geometry,
    basis: &BasisFile,
    method: HfMethod,
    multiplicity: u8,
) -> Result<CalculationResult, TestCaseError> {
    let result = CalculationBuilder::new(geometry, basis)
        .with_molecule_config(MoleculeConfig {
            units: Units::Angstrom,
            multiplicity: NonZeroU8::new(multiplicity)
                .expect("fixture multiplicity must be positive")
                .into(),
            ..Default::default()
        })
        .with_hf(HfConfig {
            method: method.into(),
            convergence_threshold: SCF_CONVERGENCE_THRESHOLD
                .try_into()
                .expect("positive finite SCF threshold"),
            ..Default::default()
        })
        .with_mp2(Mp2Config::default())
        .execute()
        .map_err(|error| {
            let diagnostics = error.hf().map(|hf| {
                let scf = &hf.summary().scf;
                (scf.iterations, scf.delta_energy, scf.residual_norm)
            });
            TestCaseError::fail(format!(
                "calculation failed: {error}; SCF (iterations, energy change, residual)={diagnostics:?}"
            ))
        })?;
    let scf = &result.hf.summary().scf;
    prop_assert!(
        matches!(result.hf, HfOutcome::Converged(_)),
        "SCF did not converge: iterations={}, energy change={:e}, residual={:e}",
        scf.iterations,
        scf.delta_energy,
        scf.residual_norm
    );
    prop_assert!(result.mp2.is_some());
    Ok(result)
}

struct ReferenceCase {
    name: &'static str,
    geometry: &'static Geometry,
    method: HfMethod,
    multiplicity: u8,
    result: CalculationResult,
}

// Only fixtures are shared. Each generated geometry gets a fresh calculation.
#[fixture]
#[once]
fn references(
    basis: &BasisFile,
    h2o_geometry: &'static Geometry,
    oh_geometry: &'static Geometry,
) -> [ReferenceCase; 2] {
    [
        ("H2O/RHF", h2o_geometry, HfMethod::Rhf, 1),
        ("OH/UHF", oh_geometry, HfMethod::Uhf, 2),
    ]
    .map(|(name, geometry, method, multiplicity)| ReferenceCase {
        name,
        geometry,
        method,
        multiplicity,
        result: calculate(geometry, basis, method, multiplicity)
            .expect("reference calculation must converge"),
    })
}

#[fixture]
fn translations() -> impl Strategy<Value = Vector3<f64>> {
    // Moderate displacements in Angstrom avoid loss of coordinate precision.
    prop::array::uniform3(-5.0_f64..5.0).prop_map(Vector3::from)
}

#[fixture]
fn rotations() -> impl Strategy<Value = Rotation3<f64>> {
    // Euler angles cover general proper rotations. Shrinking toward zero keeps
    // rotations valid and tends toward the identity; Haar sampling is not needed.
    prop::array::uniform3(-PI..PI)
        .prop_map(|angles| Rotation3::from_euler_angles(angles[0], angles[1], angles[2]))
}

fn check_rigid_motion(
    references: &[ReferenceCase; 2],
    basis: &BasisFile,
    translation: Vector3<f64>,
    rotation: Rotation3<f64>,
) -> TestCaseResult {
    // Exercise both molecular references for every generated transformation.
    for reference in references {
        let mut geometry = reference.geometry.clone();
        let transformation = Isometry3::new(translation, rotation.scaled_axis());
        geometry.transform(&transformation);
        let result = calculate(&geometry, basis, reference.method, reference.multiplicity)
            .map_err(|error| {
                TestCaseError::fail(format!(
                    "{}: {error}; translation={translation:?}, rotation={rotation:?}",
                    reference.name
                ))
            })?;
        let scf = &result.hf.summary().scf;
        let expected_scf = &reference.result.hf.summary().scf;
        let check = |name: &str, actual: f64, expected: f64| -> TestCaseResult {
            let difference = (actual - expected).abs();
            prop_assert!(
                difference.is_finite()
                    && approx::abs_diff_eq!(actual, expected, epsilon = ABSOLUTE_TOLERANCE),
                "{}: {name}: actual={actual:e}, expected={expected:e}, \
                absolute difference={difference:e}, tolerance={ABSOLUTE_TOLERANCE:e}; \
                SCF iterations={}, residual={:e}; reference SCF iterations={}, residual={:e}; \
                translation={translation:?}, rotation={rotation:?}",
                reference.name,
                scf.iterations,
                scf.residual_norm,
                expected_scf.iterations,
                expected_scf.residual_norm
            );
            Ok(())
        };
        check(
            "nuclear repulsion",
            scf.nuclear_repulsion_energy,
            expected_scf.nuclear_repulsion_energy,
        )?;
        check(
            "HF electronic energy",
            scf.electronic_energy,
            expected_scf.electronic_energy,
        )?;
        check(
            "HF total energy",
            scf.total_energy,
            expected_scf.total_energy,
        )?;
        check(
            "kinetic energy",
            scf.energy_details.kinetic_energy,
            expected_scf.energy_details.kinetic_energy,
        )?;
        check(
            "nuclear attraction",
            scf.energy_details.nuclear_attraction_energy,
            expected_scf.energy_details.nuclear_attraction_energy,
        )?;
        check(
            "electron repulsion",
            scf.energy_details.electron_repulsion_energy,
            expected_scf.energy_details.electron_repulsion_energy,
        )?;
        let mp2 = result.mp2.as_ref().expect("calculation must include MP2");
        let expected_mp2 = reference
            .result
            .mp2
            .as_ref()
            .expect("reference calculation must include MP2");
        check(
            "MP2 correlation energy",
            mp2.correlation_energy,
            expected_mp2.correlation_energy,
        )?;
        check(
            "MP2 electronic energy",
            mp2.electronic_energy,
            expected_mp2.electronic_energy,
        )?;
        check(
            "MP2 total energy",
            mp2.electronic_energy + scf.nuclear_repulsion_energy,
            expected_mp2.electronic_energy + expected_scf.nuclear_repulsion_energy,
        )?;
        prop_assert!(
            scf.spin.is_some() == expected_scf.spin.is_some(),
            "{}: spin observables must match reference presence; \
            translation={translation:?}, rotation={rotation:?}",
            reference.name
        );
        if let (Some(spin), Some(expected_spin)) = (scf.spin, expected_scf.spin) {
            check("spin squared", spin.s_squared, expected_spin.s_squared)?;
            check(
                "spin contamination",
                spin.spin_contamination,
                expected_spin.spin_contamination,
            )?;
        }
    }
    Ok(())
}

#[fixture]
fn property_config() -> ProptestConfig {
    let mut config = ProptestConfig::default();
    // Each case runs RHF/MP2 and UHF/MP2. Keep ordinary runs small, while allowing
    // extended runs with PROPTEST_CASES and reproduction with PROPTEST_RNG_SEED.
    if std::env::var_os("PROPTEST_CASES").is_none() {
        config.cases = 16;
    }
    config
}

#[rstest]
#[case::translation(translations::default(), Just(Rotation3::identity()))]
#[case::rotation(Just(Vector3::zeros()), rotations::default())]
#[case::combined(translations::default(), rotations::default())]
fn scalar_observables_are_rigid_motion_invariant(
    basis: &BasisFile,
    references: &[ReferenceCase; 2],
    property_config: ProptestConfig,
    #[case] translations: impl Strategy<Value = Vector3<f64>>,
    #[case] rotations: impl Strategy<Value = Rotation3<f64>>,
) {
    proptest!(property_config, |(translation in translations, rotation in rotations)| {
        check_rigid_motion(references, basis, translation, rotation)?;
    });
}

#[rstest]
#[case::identity(Rotation3::identity())]
#[case::x_90_degrees(Rotation3::from_axis_angle(&Vector3::x_axis(), FRAC_PI_2))]
#[case::y_90_degrees(Rotation3::from_axis_angle(&Vector3::y_axis(), FRAC_PI_2))]
#[case::z_90_degrees(Rotation3::from_axis_angle(&Vector3::z_axis(), FRAC_PI_2))]
#[case::x_180_degrees(Rotation3::from_axis_angle(&Vector3::x_axis(), PI))]
#[case::y_180_degrees(Rotation3::from_axis_angle(&Vector3::y_axis(), PI))]
#[case::z_180_degrees(Rotation3::from_axis_angle(&Vector3::z_axis(), PI))]
fn scalar_observables_are_cardinal_rotation_invariant(
    basis: &BasisFile,
    references: &[ReferenceCase; 2],
    #[case] rotation: Rotation3<f64>,
) {
    check_rigid_motion(references, basis, Vector3::zeros(), rotation)
        .expect("cardinal rotations must preserve scalar observables");
}
