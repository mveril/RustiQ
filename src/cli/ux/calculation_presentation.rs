use rustiq_core::{
    calculation::{CalculationRequest, PreparedCalculation},
    molecules::units::Units,
};

pub(crate) fn requested_geometry(request: &CalculationRequest) -> String {
    request.geometry().to_string()
}

pub(crate) fn resolved_calculation(prepared: &PreparedCalculation) -> String {
    let molecule = prepared.get_molecule();
    let multiplicity = molecule.multiplicity().get();
    let mut geometry = molecule.geometry().clone();
    geometry.comment = "Resolved geometry".into();
    format!(
        "Resolved calculation\n  Coordinates  Bohr\n  Charge       {}\n  Multiplicity {}\n  HF method    {}\n  Basis        {} ({} functions)\n\nResolved geometry (XYZ, Bohr)\n{}",
        molecule.charge(),
        multiplicity,
        prepared.hf_method(),
        prepared.request().basis_name(),
        prepared.get_basis().nbasis(),
        geometry,
    )
}

pub(crate) fn requested_heading(unit: Units) -> String {
    format!("Requested geometry (XYZ, {})", unit_name(unit))
}

fn unit_name(unit: Units) -> &'static str {
    match unit {
        Units::Bohr => "Bohr",
        Units::Angstrom => "Angstrom",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustiq_core::{
        basis::BasisFile,
        calculation::CalculationBuilder,
        config::{HfConfig, MoleculeConfig},
        molecules::{geometry::Geometry, units::Units},
    };

    #[test]
    fn prepared_views_render_requested_and_resolved_geometry_without_source_files() {
        let geometry =
            Geometry::from_reader(&include_bytes!("../../../samples/h2/molecule.xyz")[..]).unwrap();
        let basis =
            BasisFile::from_reader(&include_bytes!("../../../tests/data/sto-3g.json")[..]).unwrap();
        let prepared = CalculationBuilder::new(&geometry, &basis)
            .with_molecule_config(MoleculeConfig {
                units: Units::Angstrom,
                ..Default::default()
            })
            .with_hf(HfConfig::default())
            .prepare()
            .unwrap();
        drop(geometry);

        assert!(requested_geometry(prepared.request()).contains("0.370000"));
        assert!(requested_geometry(prepared.request()).contains("Requested geometry"));
        assert!(!requested_geometry(prepared.request()).contains("Hydrogen molecule"));
        assert!(requested_heading(prepared.request().molecule().units).contains("Angstrom"));
        let resolved = resolved_calculation(&prepared);
        assert!(resolved.contains("HF method    RHF"));
        assert!(resolved.contains("Coordinates  Bohr"));
        assert!(resolved.contains("0.699199"));
    }

    #[test]
    fn normalized_request_drops_input_source_spans_and_preserves_auto() {
        let geometry =
            Geometry::from_reader(&include_bytes!("../../../samples/h2/molecule.xyz")[..]).unwrap();
        let basis =
            BasisFile::from_reader(&include_bytes!("../../../tests/data/sto-3g.json")[..]).unwrap();
        let mut hf = HfConfig::default();
        hf.method.value = rustiq_core::config::HfMethod::Auto;
        hf.method.span = Some((2, 3).into());
        let prepared = CalculationBuilder::new(&geometry, &basis)
            .with_hf(hf)
            .prepare()
            .unwrap();

        assert_eq!(
            prepared.request().hf().method.value,
            rustiq_core::config::HfMethod::Auto
        );
        assert!(prepared.request().hf().method.span.is_none());
        assert_eq!(prepared.hf_method().to_string(), "RHF");
    }
}
