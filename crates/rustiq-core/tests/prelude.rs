use rustiq_core::prelude::*;

#[test]
fn prelude_supports_builder_and_prepared_execution() {
    let geometry =
        Geometry::from_reader(&include_bytes!("data/samples/h2/molecule.xyz")[..]).unwrap();
    let basis = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let builder = CalculationBuilder::new(&geometry, &basis)
        .with_molecule_config(MoleculeConfig {
            units: Units::Angstrom,
            ..Default::default()
        })
        .with_hf(HfConfig {
            method: HfMethod::Rhf.into(),
            ..Default::default()
        })
        .with_mp2(Mp2Config::default());
    let direct: Result<CalculationResult, CalculationExecutionError> = builder.execute();
    let direct = direct.unwrap();
    let mut mutable = CalculationBuilder::new(&geometry, &basis);
    mutable
        .molecule_config(*builder.get_molecule_config())
        .hf(builder.get_hf().clone())
        .integrals(IntegralConfig::default())
        .mp2(*builder.get_mp2().unwrap());
    let mutable_result = mutable.execute().unwrap();
    assert_eq!(direct.mp2, mutable_result.mp2);
    let prepared: PreparedCalculation = builder.prepare().unwrap();
    let result = prepared.execute().unwrap();
    let hf: AnyHfSolution<Converged> = result.hf.into_converged().unwrap();
    let typed = hf.as_restricted().unwrap();
    let typed: &RhfSolution = typed;
    let coefficients: &MoCoefficients<Restricted> = typed.mo_coefficients();
    let energies: &OrbitalEnergies<Restricted> = typed.orbital_energies();
    let occupied: &Occupations<Restricted> = typed.occupied_orbitals();
    assert_eq!(coefficients.ncols(), energies.len());
    assert!(*occupied <= coefficients.ncols());
    assert_eq!(direct.mp2, result.mp2);
    assert_eq!(Some(hf.mp2(Mp2Config::default()).unwrap()), result.mp2);
}
