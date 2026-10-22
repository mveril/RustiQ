//! Rigid motions preserve scalar observables, including with oriented p orbitals.
use std::{
    f64::consts::{FRAC_PI_2, PI},
    num::NonZeroU8,
    sync::LazyLock,
};

use nalgebra::{Rotation3, Vector3};
use proptest::{
    prelude::*,
    test_runner::{TestCaseResult, TestRunner},
};
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

static BASIS: LazyLock<BasisFile> = LazyLock::new(|| {
    BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..])
        .expect("STO-3G fixture must be valid")
});

fn calculate(
    geometry: &Geometry,
    method: HfMethod,
    multiplicity: u8,
) -> Result<CalculationResult, TestCaseError> {
    let result = CalculationBuilder::new(geometry, &BASIS)
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
    scf_iterations: usize,
    scf_residual_norm: f64,
}

impl ReferenceCase {
    fn new(name: &'static str, xyz: &[u8], method: HfMethod, multiplicity: u8) -> Self {
        let geometry = Geometry::from_reader(xyz).expect("geometry fixture must be valid");
        let result = calculate(&geometry, method, multiplicity)
            .expect("reference calculation must converge");
        let observables = observables(&result);
        let scf = &result.hf.summary().scf;
        Self {
            name,
            geometry,
            method,
            multiplicity,
            observables,
            scf_iterations: scf.iterations,
            scf_residual_norm: scf.residual_norm,
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
        let result =
            calculate(&geometry, reference.method, reference.multiplicity).map_err(|error| {
                TestCaseError::fail(format!(
                    "{}: {error}; translation={translation:?}, rotation={rotation:?}",
                    reference.name
                ))
            })?;
        let scf = &result.hf.summary().scf;
        let actual = observables(&result);
        prop_assert_eq!(actual.len(), reference.observables.len());
        for ((name, actual), (expected_name, expected)) in actual.iter().zip(&reference.observables)
        {
            prop_assert_eq!(name, expected_name);
            let difference = (actual - expected).abs();
            prop_assert!(
                difference < ABSOLUTE_TOLERANCE,
                "{}: {name}: actual={actual:e}, expected={expected:e}, \
                absolute difference={difference:e}, tolerance={ABSOLUTE_TOLERANCE:e}; \
                SCF iterations={}, residual={:e}; reference SCF iterations={}, residual={:e}; \
                translation={translation:?}, rotation={rotation:?}",
                reference.name,
                scf.iterations,
                scf.residual_norm,
                reference.scf_iterations,
                reference.scf_residual_norm
            );
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

// A fixed batch guarantees every cardinal orientation is exercised. Configure
// the runner directly so PROPTEST_CASES cannot multiply this constant batch.
#[test]
fn scalar_observables_are_cardinal_rotation_invariant() {
    let angles = Just([
        [0.0, 0.0, 0.0],
        [FRAC_PI_2, 0.0, 0.0],
        [0.0, FRAC_PI_2, 0.0],
        [0.0, 0.0, FRAC_PI_2],
        [PI, 0.0, 0.0],
        [0.0, PI, 0.0],
        [0.0, 0.0, PI],
    ]);
    let mut runner = TestRunner::new(ProptestConfig {
        cases: 1,
        source_file: Some(file!()),
        test_name: Some(concat!(
            module_path!(),
            "::scalar_observables_are_cardinal_rotation_invariant"
        )),
        ..ProptestConfig::default()
    });
    runner
        .run(&angles, |angles| {
            // Axis permutations and sign changes exercise Cartesian p orbitals.
            for [roll, pitch, yaw] in angles {
                check_rigid_motion(
                    Vector3::zeros(),
                    Rotation3::from_euler_angles(roll, pitch, yaw),
                )?;
            }
            Ok(())
        })
        .expect("cardinal rotations must preserve scalar observables");
}
