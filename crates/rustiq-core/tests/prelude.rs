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
    let prepared: PreparedCalculation = builder.prepare().unwrap();
    let result = prepared.execute().unwrap();
    let hf: HfSolution = result.hf;
    assert!(hf.summary().scf.converged);
    assert_eq!(direct.mp2, result.mp2);
    assert_eq!(Some(hf.mp2(Mp2Config::default()).unwrap()), result.mp2);
}
