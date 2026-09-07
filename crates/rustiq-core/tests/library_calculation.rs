use std::num::NonZeroU8;

use approx::assert_abs_diff_eq;
use rustiq_core::{
    basis::{gaussian::basis::Basis, BasisFile},
    hf::{scf::ScfCalculation, uhf::UhfCalculation},
    molecules::{geometry::Geometry, molecule::Molecule, units::Units},
    mp2,
    runfile::hf::DensityGuessConfig,
};

// Exercise the same calculation types as an external consumer, without access
// to crate-private helpers or the optional benchmark interface.
#[test]
fn library_runs_hf_and_mp2_with_public_types() {
    let geometry =
        Geometry::from_reader(&include_bytes!("../../../samples/h2/molecule.xyz")[..]).unwrap();
    let mut molecule = Molecule::try_new(geometry, Units::Angstrom, 0, NonZeroU8::MIN).unwrap();
    molecule.convert_to(Units::Bohr);
    let file =
        BasisFile::from_reader(&include_bytes!("../../../tests/data/sto-3g.json")[..]).unwrap();
    let basis = Basis::try_load(&file, &molecule).unwrap();

    let mut rhf = ScfCalculation::new(
        &molecule,
        &basis,
        100,
        1e-12,
        1e-8,
        DensityGuessConfig::default(),
    )
    .unwrap();
    let rhf_result = rhf.run().unwrap();
    assert!(rhf_result.converged);
    assert_abs_diff_eq!(
        rhf_result.electronic_energy,
        -1.831_863_646_477_507,
        epsilon = 1e-10
    );
    let rhf_mp2 = mp2::rhf_closed_shell(&rhf, 0).unwrap();
    assert_abs_diff_eq!(
        rhf_mp2.correlation_energy,
        -0.013_138_073_589_533,
        epsilon = 1e-11
    );

    let mut uhf = UhfCalculation::new(
        &molecule,
        &basis,
        100,
        1e-12,
        1e-8,
        DensityGuessConfig::default(),
    )
    .unwrap();
    let uhf_result = uhf.run().unwrap();
    assert!(uhf_result.converged);
    assert_abs_diff_eq!(
        uhf_result.electronic_energy,
        rhf_result.electronic_energy,
        epsilon = 1e-10
    );
    assert_abs_diff_eq!(
        mp2::uhf_unrestricted(&uhf, 0).unwrap().correlation_energy,
        rhf_mp2.correlation_energy,
        epsilon = 1e-11
    );
}
