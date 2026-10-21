//! Test-only bridge to the production integral routines; no public export API.
use crate::{
    basis::{Basis, BasisFile},
    eri::{electron_repulsion_ints_with_threshold, index::PairIndex},
    hf::core::nucl_attraction_ints,
    molecules::{geometry::Geometry, molecule::Molecule, units::Units},
};
use nalgebra::{Rotation3, Vector3};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fmt::Write;
use std::{fs::File, num::NonZeroU8, path::Path};

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut output, byte| {
            write!(output, "{byte:02x}").unwrap();
            output
        })
}

#[rstest::rstest]
#[case("h2-sto-3g", "samples/h2/molecule.xyz", "sto-3g", false, 2)]
#[case("h2o-sto-3g", "samples/h2o/h2o.xyz", "sto-3g", false, 7)]
#[case("h2o-6-31g", "samples/h2o/h2o.xyz", "6-31g", false, 13)]
#[case("h2o-cc-pvdz", "samples/h2o/h2o.xyz", "cc-pvdz", false, 24)]
#[case("h2o-cc-pvdz-cart", "samples/h2o/h2o.xyz", "cc-pvdz", true, 25)]
#[case("h2o-cc-pvdz-cart-rotated", "samples/h2o/h2o.xyz", "cc-pvdz", true, 25)]
#[allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "Keep the five-case scientific fixture comparison and standard integral indices together"
)]
fn export_integral_validation(
    #[case] name: &str,
    #[case] geometry: &str,
    #[case] basis_name: &str,
    #[case] cartesian: bool,
    #[case] dimension: usize,
) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture: serde_json::Value = serde_json::from_reader(
        File::open(root.join("crates/rustiq-core/tests/data/integrals/pyscf-v1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(fixture["schema_version"], 1);
    assert_eq!(fixture["absolute_tolerance"], 1e-10);
    let mut failures = Vec::new();
    let mut molecule_geometry =
        Geometry::from_file(File::open(root.join(geometry)).unwrap()).unwrap();
    if name == "h2o-cc-pvdz-cart-rotated" {
        let axis = nalgebra::Unit::new_normalize(Vector3::new(1.0, 2.0, 3.0));
        molecule_geometry.rotate(Rotation3::from_axis_angle(&axis, 0.37));
    }
    let mut molecule =
        Molecule::try_new(molecule_geometry, Units::Angstrom, 0, NonZeroU8::MIN).unwrap();
    molecule.convert_to(Units::Bohr);
    let reference = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["case_id"] == name)
        .unwrap();
    for (path, key) in [
        (geometry.to_owned(), "geometry_sha256"),
        (
            format!("tests/data/reference/RustiQ/basis_sets/{basis_name}.json"),
            "basis_sha256",
        ),
    ] {
        let hash = sha256(&std::fs::read(root.join(path)).unwrap());
        assert_eq!(reference[key], hash, "stale integral fixture for {name}");
    }
    let mut source: serde_json::Value = serde_json::from_reader(
        File::open(root.join(format!(
            "tests/data/reference/RustiQ/basis_sets/{basis_name}.json"
        )))
        .unwrap(),
    )
    .unwrap();
    if cartesian {
        for element in source["elements"].as_object_mut().unwrap().values_mut() {
            for shell in element["electron_shells"].as_array_mut().unwrap() {
                shell["function_type"] = json!("gto_cartesian");
            }
        }
    }
    let file: BasisFile = serde_json::from_value(source).unwrap();
    let basis = Basis::try_load(&file, molecule.geometry()).unwrap();
    assert_eq!(basis.nbasis(), dimension);
    let s = basis.overlap_ints();
    let t = basis.kinetic_ints();
    let v = nucl_attraction_ints(molecule.geometry(), &basis);
    let h = &t + &v;
    let eri = electron_repulsion_ints_with_threshold(&basis, None).unwrap();
    for (offset, &value) in eri.ordered_values().iter().enumerate() {
        assert!(value.is_finite());
        let (p, q) = PairIndex(offset).indices();
        let (i, j) = PairIndex(p).indices();
        let (k, l) = PairIndex(q).indices();
        for indices in [
            (i, j, k, l),
            (j, i, k, l),
            (i, j, l, k),
            (j, i, l, k),
            (k, l, i, j),
            (l, k, i, j),
            (k, l, j, i),
            (l, k, j, i),
        ] {
            assert_eq!(eri[indices], value);
        }
    }
    for (family, values) in [
        ("S", s.as_slice()),
        ("T", t.as_slice()),
        ("V", v.as_slice()),
        ("Hcore", h.as_slice()),
        ("ERI", eri.ordered_values()),
    ] {
        let array_name = reference["arrays"][family].as_str().unwrap();
        let bytes = std::fs::read(root.join(format!(
            "crates/rustiq-core/tests/data/integrals/{array_name}.npy"
        )))
        .unwrap();
        assert_eq!(fixture["array_sha256"][array_name], sha256(&bytes));
        let expected = if family == "ERI" {
            crate::persistence::read_compact_eri(bytes.as_slice(), dimension)
                .unwrap()
                .ordered_values()
                .to_vec()
        } else {
            let matrix = crate::persistence::read_dmatrix(bytes.as_slice()).unwrap();
            assert_eq!(matrix.shape(), (dimension, dimension));
            matrix.as_slice().to_vec()
        };
        assert_eq!(values.len(), expected.len());
        let mut worst = (0, 0.0_f64);
        for (index, (&value, reference)) in values.iter().zip(&expected).enumerate() {
            let reference = *reference;
            assert!(value.is_finite() && reference.is_finite());
            let error = (value - reference).abs();
            if error > worst.1 {
                worst = (index, error);
            }
        }
        if worst.1 > 1e-10 {
            failures.push(format!(
                "{name} {family}: offset {}, RustiQ {}, PySCF {}, absolute error {} exceeds 1e-10",
                worst.0, values[worst.0], expected[worst.0], worst.1
            ));
        }
    }
    // nalgebra's contiguous matrix storage is column-major.
    let result = json!({"case_id": name, "nao": dimension,
            "S": s.as_slice(), "T": t.as_slice(), "V": v.as_slice(),
            "Hcore": h.as_slice(), "ERI": eri.ordered_values()});
    if let Some(path) = std::env::var_os("RUSTIQ_INTEGRAL_OUTPUT") {
        let path = Path::new(&path).join(format!("{name}.json"));
        std::fs::write(path, serde_json::to_vec(&result).unwrap()).unwrap();
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
