use approx::assert_abs_diff_eq;
use rustiq_core::{
    basis::BasisFile,
    calculation::{CalculationBuilder, CalculationExecution},
    config::{HfConfig, HfMethod, Mp2Config},
    molecules::{geometry::Geometry, units::Units},
};

// Exercise the same calculation types as an external consumer, without access
// to crate-private helpers or the optional benchmark interface.
#[test]
fn library_runs_hf_and_mp2_with_public_types() {
    let geometry =
        Geometry::from_reader(&include_bytes!("data/samples/h2/molecule.xyz")[..]).unwrap();
    let file = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let rhf_result = CalculationBuilder::new(&geometry, &file)
        .with_molecule_config(rustiq_core::config::MoleculeConfig {
            units: Units::Angstrom,
            ..Default::default()
        })
        .with_hf(HfConfig {
            method: HfMethod::Rhf.into(),
            ..Default::default()
        })
        .with_mp2(Mp2Config::default())
        .execute()
        .unwrap();
    assert!(rhf_result.hf.scf.converged);
    assert_abs_diff_eq!(
        rhf_result.hf.scf.electronic_energy,
        -1.831_863_646_477_507,
        epsilon = 1e-10
    );
    assert_abs_diff_eq!(
        rhf_result.mp2.unwrap().correlation_energy,
        -0.013_138_073_589_533,
        epsilon = 1e-11
    );

    let uhf_result = CalculationBuilder::new(&geometry, &file)
        .with_molecule_config(rustiq_core::config::MoleculeConfig {
            units: Units::Angstrom,
            ..Default::default()
        })
        .with_hf(HfConfig {
            method: HfMethod::Uhf.into(),
            ..Default::default()
        })
        .with_mp2(Mp2Config::default())
        .execute()
        .unwrap();
    assert!(uhf_result.hf.scf.converged);
    assert_abs_diff_eq!(
        uhf_result.hf.scf.electronic_energy,
        rhf_result.hf.scf.electronic_energy,
        epsilon = 1e-10
    );
    assert_abs_diff_eq!(
        uhf_result.mp2.unwrap().correlation_energy,
        -0.013_138_073_589_533,
        epsilon = 1e-11
    );
}
