use super::element_ext::{AtomicMassParseError, ElementExt};
use super::{atom::Atom, geometry_parse_error::GeometryParseError, xyz_parser::parse_xyz};
use core::iter::Iterator;
use delegate::delegate;
use nalgebra::{distance, Isometry3, Matrix3, Point3, Rotation3, Translation3, Vector3};
use rayon::iter::{IntoParallelIterator, ParallelBridge, ParallelIterator};
use std::ops::{Index, IndexMut, Range};
use std::path::Path;
#[allow(dead_code)]
use std::{
    fmt::{self, Display},
    fs::File,
    io::{BufRead, BufReader, Cursor, Write},
    str::FromStr,
};

#[derive(Debug, Clone)]
pub struct Geometry {
    pub comment: String,
    pub atoms: Vec<Atom>,
}

impl Geometry {
    pub fn new(comment: String, atoms: Vec<Atom>) -> Self {
        Geometry { comment, atoms }
    }

    pub fn from_source(
        source_name: impl Into<String>,
        source: &str,
    ) -> Result<Self, GeometryParseError> {
        parse_xyz(source_name, source)
    }

    pub fn from_reader(mut reader: impl BufRead) -> Result<Self, GeometryParseError> {
        let mut source = String::new();
        reader.read_to_string(&mut source)?;
        Self::from_source("<geometry>", &source)
    }

    pub fn from_file(file: File) -> Result<Self, GeometryParseError> {
        Self::from_reader(BufReader::new(file))
    }

    pub fn from_path(path: &Path) -> Result<Self, GeometryParseError> {
        let source = std::fs::read_to_string(path)?;
        Self::from_source(path.display().to_string(), &source)
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn to_writer(self, mut writer: impl Write) -> std::io::Result<()> {
        write!(writer, "{}", self)?;
        Ok(())
    }

    pub fn nucl_repulsion(&self) -> f64 {
        self.atoms
            .iter()
            .enumerate()
            .flat_map(|(i, atom_i)| {
                self.atoms[i + 1..]
                    .iter()
                    .map(move |atom_j| (atom_i, atom_j))
            })
            .par_bridge()
            .map(|(atom_i, atom_j)| {
                let z_i = atom_i.element.atomic_number as f64;
                let z_j = atom_j.element.atomic_number as f64;
                let r_ij = distance(&atom_i.position, &atom_j.position);
                z_i * z_j / r_ij
            })
            .sum()
    }

    pub fn center(&self) -> Point3<f64> {
        (self
            .atoms
            .iter()
            .map(|a| a.position.coords)
            .sum::<Vector3<f64>>()
            / self.atoms.len() as f64)
            .into()
    }

    pub fn mass_center(&self) -> Result<Point3<f64>, AtomicMassParseError> {
        let total_mass: f64 = self
            .atoms
            .iter()
            .map(|a| a.element.atomic_mass_f64())
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .sum();
        Ok((self
            .atoms
            .iter()
            .map(|a| {
                a.element
                    .atomic_mass_f64()
                    .map(|mass| a.position.coords * mass)
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .sum::<Vector3<f64>>()
            / total_mass)
            .into())
    }

    pub fn charge_center(&self) -> Point3<f64> {
        (self
            .atoms
            .iter()
            .map(|a| a.position.coords * a.element.atomic_number as f64)
            .sum::<Vector3<f64>>()
            / self
                .atoms
                .iter()
                .map(|a| a.element.atomic_number as f64)
                .sum::<f64>())
        .into()
    }

    pub fn translate(&mut self, translation: Translation3<f64>) {
        for atom in &mut self.atoms {
            atom.position = translation * atom.position;
        }
    }

    pub fn rotate(&mut self, rotation: Rotation3<f64>) {
        for atom in &mut self.atoms {
            atom.position = rotation * atom.position;
        }
    }

    pub fn transform(&mut self, transformation: &Isometry3<f64>) {
        for atom in &mut self.atoms {
            atom.position = transformation * atom.position;
        }
    }

    pub fn centering(&mut self) {
        let center = self.center();
        let translation = Translation3::from(-center.coords);
        self.translate(translation);
    }

    pub fn mass_centering(&mut self) -> Result<(), AtomicMassParseError> {
        let mass_center = self.mass_center()?;
        let translation = Translation3::from(-mass_center.coords);
        self.translate(translation);
        Ok(())
    }

    pub fn charge_centering(&mut self) {
        let charge_center = self.charge_center();
        let translation = Translation3::from(-charge_center.coords);
        self.translate(translation);
    }

    pub fn inertia_tensor(&self) -> Result<Matrix3<f64>, AtomicMassParseError> {
        self.atoms
            .iter()
            .try_fold(Matrix3::zeros(), |tensor, atom| {
                let mass = atom.element.atomic_mass_f64()?;
                let r = atom.position.coords;
                let atom_tensor = mass * (r.dot(&r) * Matrix3::identity() - r * r.transpose());

                Ok(tensor + atom_tensor)
            })
    }

    pub fn orient_along_principal_axes(&mut self) -> Result<(), AtomicMassParseError> {
        self.mass_centering()?;

        let eigen = self.inertia_tensor()?.symmetric_eigen();
        let axes: nalgebra::Matrix<f64, nalgebra::Const<3>, nalgebra::Const<3>, nalgebra::ArrayStorage<f64, 3, 3>> = ensure_direct_basis(eigen.eigenvectors);
        let rotation = Rotation3::from_matrix_unchecked(axes.transpose());

        self.rotate(rotation);

        Ok(())
    }
}

fn ensure_direct_basis(mut axes: Matrix3<f64>) -> Matrix3<f64> {
    if axes.determinant() < 0.0 {
        axes.column_mut(2).neg_mut();
    }
    axes
}

impl FromStr for Geometry {
    type Err = GeometryParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cursor = Cursor::new(s);
        Self::from_reader(cursor)
    }
}

impl Display for Geometry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let width = f.width().unwrap_or(12);
        let precision = f.precision().unwrap_or(6);
        writeln!(f, "{}", self.atoms.len())?;
        writeln!(f, "{}", self.comment)?;
        for atom in &self.atoms {
            writeln!(
                f,
                "{:<2} {:>width$.precision$} {:>width$.precision$} {:>width$.precision$}",
                atom.element.symbol,
                atom.position.x,
                atom.position.y,
                atom.position.z,
                width = width,
                precision = precision
            )?;
        }
        Ok(())
    }
}

impl IntoIterator for Geometry {
    type Item = Atom;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    delegate! {
        to self.atoms {
            fn into_iter(self) -> Self::IntoIter;
        }
    }
}

impl<'a> IntoIterator for &'a Geometry {
    type Item = &'a Atom;
    type IntoIter = std::slice::Iter<'a, Atom>;

    fn into_iter(self) -> Self::IntoIter {
        self.atoms.iter()
    }
}

impl IntoParallelIterator for Geometry {
    type Item = Atom;
    type Iter = rayon::vec::IntoIter<Atom>;

    delegate! {
        to self.atoms {
            fn into_par_iter(self) -> Self::Iter;
        }
    }
}

impl Index<usize> for Geometry {
    type Output = Atom;

    delegate! {
        to self.atoms {
            fn index(&self, index: usize) -> &Self::Output;
        }
    }
}

impl IndexMut<usize> for Geometry {
    delegate! {
        to self.atoms {
            fn index_mut(&mut self, index: usize) -> &mut Self::Output;
        }
    }
}

impl Index<Range<usize>> for Geometry {
    type Output = [Atom];

    delegate! {
        to self.atoms {
            fn index(&self, index: Range<usize>) -> &Self::Output;
        }
    }
}

impl IndexMut<Range<usize>> for Geometry {
    delegate! {
        to self.atoms {
            fn index_mut(&mut self, index: Range<usize>) -> &mut Self::Output;
        }
    }
}

impl Extend<Atom> for Geometry {
    delegate! {
        to self.atoms {
            fn extend<T: IntoIterator<Item = Atom>>(&mut self, iter: T);
        }
    }
}

impl AsRef<Vec<Atom>> for Geometry {
    delegate! {
        to self.atoms {
            fn as_ref(&self) -> &Vec<Atom>;
        }
    }
}

impl AsMut<Vec<Atom>> for Geometry {
    delegate! {
        to self.atoms {
            fn as_mut(&mut self) -> &mut Vec<Atom>;
        }
    }
}

impl AsRef<[Atom]> for Geometry {
    delegate! {
        to self.atoms {
            fn as_ref(&self) -> &[Atom];
        }
    }
}

impl AsMut<[Atom]> for Geometry {
    delegate! {
        to self.atoms {
            fn as_mut(&mut self) -> &mut [Atom];
        }
    }
}

#[cfg(test)]
mod tests {
    use nalgebra::{point, Isometry3, Matrix3, Rotation3, Translation3, Vector3};
    use periodic_table::elements;
    use std::f64::consts::FRAC_PI_2;

    use super::*;

    fn assert_point_close(actual: Point3<f64>, expected: Point3<f64>) {
        assert!(
            (actual - expected).norm() < 1e-10,
            "expected {expected:?}, found {actual:?}"
        );
    }

    fn assert_matrix_close(actual: Matrix3<f64>, expected: Matrix3<f64>) {
        assert!(
            (actual - expected).norm() < 1e-10,
            "expected {expected:?}, found {actual:?}"
        );
    }

    #[test]
    fn test_geometry_parsing_keeps_coordinates_unchanged() {
        let xyz_data = "\
2
Hydrogen molecule (centered)
H  0.000000  0.000000  -0.370000
H  0.000000  0.000000   0.370000
";

        let cursor = Cursor::new(xyz_data);
        let geometry = Geometry::from_reader(cursor).expect("Failed to parse Geometry");

        assert_eq!(geometry.atoms.len(), 2);

        let expected_position1 = point![0.0, 0.0, -0.370000];
        let expected_position2 = point![0.0, 0.0, 0.370000];

        let atom1 = &geometry.atoms[0];
        let atom2 = &geometry.atoms[1];

        assert_eq!(atom1.element.symbol, "H");
        assert_eq!(atom2.element.symbol, "H");

        assert!((atom1.position.x - expected_position1.x).abs() < 1e-6);
        assert!((atom1.position.y - expected_position1.y).abs() < 1e-6);
        assert!((atom1.position.z - expected_position1.z).abs() < 1e-6);

        assert!((atom2.position.x - expected_position2.x).abs() < 1e-6);
        assert!((atom2.position.y - expected_position2.y).abs() < 1e-6);
        assert!((atom2.position.z - expected_position2.z).abs() < 1e-6);
    }

    #[test]
    fn test_geometry_display() {
        let point_angstrom = point!(0.0, 0.0, -0.370000);
        let atom1 = Atom::new(&elements::H, point_angstrom);
        let atom2 = Atom::new(&elements::H, Point3::new(0.0, 0.0, -point_angstrom.z));
        let geometry = Geometry::new(
            "Hydrogen molecule (centered)".to_string(),
            vec![atom1.clone(), atom2.clone()],
        );

        let expected_output = format!(
            "2\nHydrogen molecule (centered)\nH  {:>12.6} {:>12.6} {:>12.6}\nH  {:>12.6} {:>12.6} {:>12.6}\n",
            point_angstrom.x,
            point_angstrom.y,
            point_angstrom.z,
            point_angstrom.x,
            point_angstrom.y,
            -point_angstrom.z,
        );

        let output = format!("{}", geometry);
        assert_eq!(output, expected_output);
    }

    #[test]
    fn test_translate_geometry() {
        let mut geometry = Geometry::new(
            "Translate".to_string(),
            vec![
                Atom::new(&elements::H, point![0.0, 0.0, 0.0]),
                Atom::new(&elements::HE, point![1.0, 2.0, 3.0]),
            ],
        );

        geometry.translate(Translation3::new(1.0, -2.0, 0.5));

        assert_point_close(geometry.atoms[0].position, point![1.0, -2.0, 0.5]);
        assert_point_close(geometry.atoms[1].position, point![2.0, 0.0, 3.5]);
    }

    #[test]
    fn test_rotate_geometry() {
        let mut geometry = Geometry::new(
            "Rotate".to_string(),
            vec![Atom::new(&elements::H, point![1.0, 0.0, 0.0])],
        );

        geometry.rotate(Rotation3::from_axis_angle(&Vector3::z_axis(), FRAC_PI_2));

        assert_point_close(geometry.atoms[0].position, point![0.0, 1.0, 0.0]);
    }

    #[test]
    fn test_transform_geometry_with_isometry() {
        let mut geometry = Geometry::new(
            "Transform".to_string(),
            vec![Atom::new(&elements::H, point![1.0, 0.0, 0.0])],
        );
        let rotation = Rotation3::from_axis_angle(&Vector3::z_axis(), FRAC_PI_2);
        let translation = Translation3::new(0.0, 1.0, 0.0);
        let isometry = Isometry3::from_parts(translation, rotation.into());

        geometry.transform(&isometry);

        assert_point_close(geometry.atoms[0].position, point![0.0, 2.0, 0.0]);
    }

    #[test]
    fn test_centering_moves_geometric_center_to_origin() {
        let mut geometry = Geometry::new(
            "Center".to_string(),
            vec![
                Atom::new(&elements::H, point![0.0, 0.0, 0.0]),
                Atom::new(&elements::HE, point![2.0, 0.0, 0.0]),
            ],
        );

        geometry.centering();

        assert_point_close(geometry.center(), Point3::origin());
        assert_point_close(geometry.atoms[0].position, point![-1.0, 0.0, 0.0]);
        assert_point_close(geometry.atoms[1].position, point![1.0, 0.0, 0.0]);
    }

    #[test]
    fn test_mass_centering_moves_mass_center_to_origin() {
        let mut geometry = Geometry::new(
            "Mass center".to_string(),
            vec![
                Atom::new(&elements::H, point![0.0, 0.0, 0.0]),
                Atom::new(&elements::HE, point![2.0, 0.0, 0.0]),
            ],
        );

        geometry.mass_centering().unwrap();

        assert_point_close(geometry.mass_center().unwrap(), Point3::origin());
    }

    #[test]
    fn test_charge_centering_moves_charge_center_to_origin() {
        let mut geometry = Geometry::new(
            "Charge center".to_string(),
            vec![
                Atom::new(&elements::H, point![0.0, 0.0, 0.0]),
                Atom::new(&elements::HE, point![2.0, 0.0, 0.0]),
            ],
        );

        geometry.charge_centering();

        assert_point_close(geometry.charge_center(), Point3::origin());
        assert_point_close(geometry.atoms[0].position, point![-4.0 / 3.0, 0.0, 0.0]);
        assert_point_close(geometry.atoms[1].position, point![2.0 / 3.0, 0.0, 0.0]);
    }

    #[test]
    fn test_inertia_tensor_uses_mass_weighted_outer_products() {
        let geometry = Geometry::new(
            "Inertia tensor".to_string(),
            vec![
                Atom::new(&elements::H, point![1.0, 2.0, 3.0]),
                Atom::new(&elements::H, point![-2.0, 0.5, 1.0]),
            ],
        );
        let mass = elements::H.atomic_mass_f64().unwrap();
        let expected = mass
            * Matrix3::new(
                13.0 + 1.25,
                -2.0 + 1.0,
                -3.0 + 2.0,
                -2.0 + 1.0,
                10.0 + 5.0,
                -6.0 - 0.5,
                -3.0 + 2.0,
                -6.0 - 0.5,
                5.0 + 4.25,
            );

        assert_matrix_close(geometry.inertia_tensor().unwrap(), expected);
    }

    #[test]
    fn test_orient_along_principal_axes_centers_mass_and_diagonalizes_inertia() {
        let mut geometry = Geometry::new(
            "Orient".to_string(),
            vec![
                Atom::new(&elements::O, point![1.0, 2.0, 0.5]),
                Atom::new(&elements::H, point![2.2, 2.4, 1.7]),
                Atom::new(&elements::H, point![0.4, 3.1, 1.2]),
            ],
        );

        geometry.orient_along_principal_axes().unwrap();

        assert_point_close(geometry.mass_center().unwrap(), Point3::origin());
        let tensor = geometry.inertia_tensor().unwrap();
        assert!(tensor[(0, 1)].abs() < 1e-10);
        assert!(tensor[(0, 2)].abs() < 1e-10);
        assert!(tensor[(1, 2)].abs() < 1e-10);
    }

    #[test]
    fn test_orient_along_principal_axes_preserves_pairwise_distances() {
        let mut geometry = Geometry::new(
            "Orient distances".to_string(),
            vec![
                Atom::new(&elements::C, point![1.0, -0.4, 0.2]),
                Atom::new(&elements::O, point![0.2, 1.3, -0.6]),
                Atom::new(&elements::H, point![-0.8, 0.1, 1.5]),
                Atom::new(&elements::H, point![1.9, 0.6, 1.1]),
            ],
        );
        let original_distances = pairwise_distances(&geometry);

        geometry.orient_along_principal_axes().unwrap();

        let oriented_distances = pairwise_distances(&geometry);
        assert_eq!(oriented_distances.len(), original_distances.len());
        for (actual, expected) in oriented_distances.into_iter().zip(original_distances) {
            assert!((actual - expected).abs() < 1e-10);
        }
    }

    fn pairwise_distances(geometry: &Geometry) -> Vec<f64> {
        geometry
            .atoms
            .iter()
            .enumerate()
            .flat_map(|(i, atom_i)| {
                geometry.atoms[i + 1..]
                    .iter()
                    .map(move |atom_j| distance(&atom_i.position, &atom_j.position))
            })
            .collect()
    }

    #[test]
    fn test_nucl_repulsion_h2() {
        // Create a simple geometry for the H2 molecule
        let atom1 = Atom::new(&elements::H, point![0.0, 0.0, 0.0]);
        let atom2 = Atom::new(&elements::H, point![0.0, 0.0, 1.40]);

        let geometry = Geometry::new("Hydrogen molecule (H2)".to_string(), vec![atom1, atom2]);

        // Calculate nucleus-nucleus repulsion for H2
        let e_nuc_nuc = geometry.nucl_repulsion();

        // Expected value for H2 at this distance
        // The distance in Bohr is ~1.40 Bohr (0.74 Angstrom)
        let expected_e_nuc_nuc = 1.0 / 1.40; // Z_H * Z_H / R_HH

        // Check that the calculated energy is close to the expected value
        assert!(
            (e_nuc_nuc - expected_e_nuc_nuc).abs() < 1e-6,
            "Erreur dans E_nuc-nuc pour H2: expected {} found {}",
            expected_e_nuc_nuc,
            e_nuc_nuc
        );
    }

    #[test]
    fn test_nucl_repulsion_h3() {
        // Create a geometry for a hypothetical linear H3 system
        let atom1 = Atom::new(&elements::H, point![0.0, 0.0, -1.0]);
        let atom2 = Atom::new(&elements::H, point![0.0, 0.0, 0.0]);
        let atom3 = Atom::new(&elements::H, point![0.0, 0.0, 1.0]);

        let geometry = Geometry::new(
            "Hydrogen molecule (H3)".to_string(),
            vec![atom1, atom2, atom3],
        );

        // Calculate nucleus-nucleus repulsion for H3
        let e_nuc_nuc = geometry.nucl_repulsion();

        // The distances are all 1 Bohr here
        let expected_e_nuc_nuc = (1.0 / 1.0) + (1.0 / 2.0) + (1.0 / 1.0);

        // Check that the calculated energy is close to the expected value
        assert!(
            (e_nuc_nuc - expected_e_nuc_nuc).abs() < 1e-6,
            "Erreur dans E_nuc-nuc pour H3: expected {} found {}",
            expected_e_nuc_nuc,
            e_nuc_nuc
        );
    }

    #[test]
    fn test_nucl_repulsion_he_h2() {
        // Create a geometry for a system with one He atom and H2
        let atom_he = Atom::new(&elements::HE, point![0.0, 0.0, -2.0]);
        let atom_h1 = Atom::new(&elements::H, point![0.0, 0.0, 0.0]);
        let atom_h2 = Atom::new(&elements::H, point![0.0, 0.0, 1.0]);

        let geometry = Geometry::new(
            "Helium and Hydrogen molecule".to_string(),
            vec![atom_he, atom_h1, atom_h2],
        );

        // Calculate nucleus-nucleus repulsion
        let e_nuc_nuc = geometry.nucl_repulsion();

        // The distances are He-H1 = 2 Bohr, He-H2 = 3 Bohr, H1-H2 = 1 Bohr
        let expected_e_nuc_nuc = (2.0 * 1.0 / 2.0) + (2.0 * 1.0 / 3.0) + (1.0 * 1.0 / 1.0);

        // Check that the calculated energy is close to the expected value
        assert!(
            (e_nuc_nuc - expected_e_nuc_nuc).abs() < 1e-6,
            "Erreur dans E_nuc-nuc pour He-H2: expected {} found {}",
            expected_e_nuc_nuc,
            e_nuc_nuc
        );
    }
}
