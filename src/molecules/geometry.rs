use super::element_ext::{AtomicMassParseError, ElementExt};
use super::{
    atom::Atom, convert_length::convert_length, element_parser::parse_element,
    geometry_parse_error::GeometryParseError, units::Units,
};
use core::iter::Iterator;
use nalgebra::{distance, Point3, Rotation3, Translation3, Vector3};
use rayon::iter::{IntoParallelIterator, ParallelBridge, ParallelIterator};
use std::ops::{Index, IndexMut, Range};
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
    pub display_unit: Units,
    pub unit: Units,
}

fn read_atom_line(
    line: &str,
    atom_index: usize,
    disp_unit: Units,
    internal_unit: Units,
) -> Result<Atom, GeometryParseError> {
    let mut parts = line.split_whitespace();
    let mut next_part = || {
        parts
            .next()
            .ok_or(GeometryParseError::AtomLineShouldHaveFourParts(
                atom_index,
                line.to_string(),
            ))
    };
    let [element_str, x_str, y_str, z_str] =
        [next_part()?, next_part()?, next_part()?, next_part()?];

    if parts.next().is_some() {
        return Err(GeometryParseError::AtomLineShouldHaveFourParts(
            atom_index,
            line.to_string(),
        ));
    };
    let element = parse_element(element_str).map_err(|err| {
        GeometryParseError::AtomLineElementError(atom_index, line.to_string(), err)
    })?;

    let parse_coordinate = |value: &str| {
        value.parse::<f64>().map_err(|err| {
            GeometryParseError::AtomLineCoordinateError(atom_index, line.to_string(), err)
        })
    };
    let mut position = Point3::new(
        parse_coordinate(x_str)?,
        parse_coordinate(y_str)?,
        parse_coordinate(z_str)?,
    );
    if disp_unit != internal_unit {
        position = convert_length(position, disp_unit, internal_unit);
    }
    Ok(Atom::new(element, position))
}

impl Geometry {
    pub fn new(
        comment: String,
        atoms: Vec<Atom>,
        display_unit: Option<Units>,
        unit: Option<Units>,
    ) -> Self {
        Geometry {
            comment,
            atoms,
            display_unit: display_unit.unwrap_or(Units::Angstrom),
            unit: unit.unwrap_or(Units::Bohr),
        }
    }

    pub fn from_reader(
        mut reader: impl BufRead,
        unit: Option<Units>,
        display_unit: Option<Units>,
    ) -> Result<Self, GeometryParseError> {
        let mut num_str = String::new();
        let internal_unit = unit.unwrap_or(Units::Bohr);
        let disp_unit = display_unit.unwrap_or(Units::Angstrom);
        reader.read_line(&mut num_str)?;
        let num = num_str
            .trim()
            .parse::<usize>()
            .map_err(GeometryParseError::ParseNumberOfAtom)?;
        let mut comm = String::new();
        reader.read_line(&mut comm)?;
        let mut atoms = Vec::<Atom>::with_capacity(num);
        for i in 0..num {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            let atom = read_atom_line(&line, i, disp_unit, internal_unit)?;
            atoms.push(atom);
        }
        Ok(Geometry::new(
            comm,
            atoms,
            Option::from(disp_unit),
            Option::from(internal_unit),
        ))
    }

    pub fn from_file(
        file: File,
        unit: Option<Units>,
        display_unit: Option<Units>,
    ) -> Result<Self, GeometryParseError> {
        Self::from_reader(BufReader::new(file), unit, display_unit)
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

    pub fn transform(&mut self, transformation: &nalgebra::Isometry3<f64>) {
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
}

impl FromStr for Geometry {
    type Err = GeometryParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cursor = Cursor::new(s);
        Self::from_reader(cursor, None, None)
    }
}

impl Display for Geometry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let width = f.width().unwrap_or(12);
        let precision = f.precision().unwrap_or(6);
        writeln!(f, "{}", self.atoms.len())?;
        writeln!(f, "{}", self.comment)?;
        for atom in &self.atoms {
            let position = convert_length(atom.position, self.unit, self.display_unit);
            writeln!(
                f,
                "{:<2} {:>width$.precision$} {:>width$.precision$} {:>width$.precision$}",
                atom.element.symbol,
                position.x,
                position.y,
                position.z,
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

    fn into_iter(self) -> Self::IntoIter {
        self.atoms.into_iter()
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

    fn into_par_iter(self) -> Self::Iter {
        self.atoms.into_par_iter()
    }
}

impl Index<usize> for Geometry {
    type Output = Atom;
    fn index(&self, index: usize) -> &Self::Output {
        &self.atoms[index]
    }
}

impl IndexMut<usize> for Geometry {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.atoms[index]
    }
}

impl Index<Range<usize>> for Geometry {
    type Output = [Atom];
    fn index(&self, index: Range<usize>) -> &Self::Output {
        &self.atoms[index]
    }
}

impl IndexMut<Range<usize>> for Geometry {
    fn index_mut(&mut self, index: Range<usize>) -> &mut Self::Output {
        &mut self.atoms[index]
    }
}

impl Extend<Atom> for Geometry {
    fn extend<T: IntoIterator<Item = Atom>>(&mut self, iter: T) {
        self.atoms.extend(iter)
    }
}

impl AsRef<Vec<Atom>> for Geometry {
    fn as_ref(&self) -> &Vec<Atom> {
        &self.atoms
    }
}

impl AsMut<Vec<Atom>> for Geometry {
    fn as_mut(&mut self) -> &mut Vec<Atom> {
        &mut self.atoms
    }
}

impl AsRef<[Atom]> for Geometry {
    fn as_ref(&self) -> &[Atom] {
        &self.atoms
    }
}

impl AsMut<[Atom]> for Geometry {
    fn as_mut(&mut self) -> &mut [Atom] {
        &mut self.atoms
    }
}

#[cfg(test)]
mod tests {
    use nalgebra::{point, Isometry3, Rotation3, Translation3, Vector3};
    use periodic_table::elements;

    use super::*;

    fn assert_point_close(actual: Point3<f64>, expected: Point3<f64>) {
        assert!(
            (actual - expected).norm() < 1e-10,
            "expected {expected:?}, found {actual:?}"
        );
    }

    #[test]
    fn test_geometry_parsing_and_conversion() {
        // Example XYZ string for H2 centered in Angstrom
        let xyz_data = "\
2
Hydrogen molecule (centered)
H  0.000000  0.000000  -0.370000
H  0.000000  0.000000   0.370000
";

        // Create a Cursor to simulate a file reader
        let cursor = Cursor::new(xyz_data);

        // Parse the geometry with display_unit = Angstrom and internal_unit = Bohr
        let geometry = Geometry::from_reader(cursor, Some(Units::Bohr), Some(Units::Angstrom))
            .expect("Failed to parse Geometry");

        // Check the number of atoms
        assert_eq!(geometry.atoms.len(), 2);

        // Check the converted coordinates
        // 1 Angstrom = 1 / 0.52917721092 Bohr, approximately 1.8897259886 Bohr
        let expected_position1 = point![0.0, 0.0, -0.370000] / 0.52917721092;
        let expected_position2 = point![0.0, 0.0, 0.370000] / 0.52917721092;

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
        // Create a simple geometry
        let point_angstrom = point!(0.0, 0.0, -0.370000);
        let point_bohr = point_angstrom * 1.8897259886;
        let atom1 = Atom::new(&elements::H, point_bohr);
        let atom2 = Atom::new(&elements::H, Point3::new(0.0, 0.0, -point_bohr.z));
        let geometry = Geometry::new(
            "Hydrogen molecule (centered)".to_string(),
            vec![atom1.clone(), atom2.clone()],
            Some(Units::Angstrom),
            Some(Units::Bohr),
        );

        // Convert coordinates for display
        let expected_output = format!(
            "2\nHydrogen molecule (centered)\nH  {:>12.6} {:>12.6} {:>12.6}\nH  {:>12.6} {:>12.6} {:>12.6}\n",
            point_angstrom.x,
            point_angstrom.y,
            point_angstrom.z,
            point_angstrom.x,
            point_angstrom.y,
            -point_angstrom.z,
        );

        // Check Display output
        let output = format!("{}", geometry);
        assert_eq!(output, expected_output);
    }

    #[test]
    fn test_convert_length() {
        // Test conversion from Bohr to Angstrom
        let position_bohr = Point3::new(1.0, 2.0, 3.0);
        let position_angstrom = convert_length(position_bohr, Units::Bohr, Units::Angstrom);

        // 1 Bohr is approximately 0.52917721092 Angstrom
        let expected_angstrom = point![1.0, 2.0, 3.0,] * 0.52917721092;

        assert!((position_angstrom.x - expected_angstrom.x).abs() < 1e-6);
        assert!((position_angstrom.y - expected_angstrom.y).abs() < 1e-6);
        assert!((position_angstrom.z - expected_angstrom.z).abs() < 1e-6);

        // Test conversion from Angstrom to Bohr
        let position_angstrom = Point3::new(0.52917721092, 1.05835442184, 1.58753163276);
        let position_bohr = convert_length(position_angstrom, Units::Angstrom, Units::Bohr);

        let expected_bohr = point![0.52917721092, 1.05835442184, 1.58753163276,] / 0.52917721092;

        assert!((position_bohr.x - expected_bohr.x).abs() < 1e-6);
        assert!((position_bohr.y - expected_bohr.y).abs() < 1e-6);
        assert!((position_bohr.z - expected_bohr.z).abs() < 1e-6);
    }

    #[test]
    fn test_translate_geometry() {
        let mut geometry = Geometry::new(
            "Translate".to_string(),
            vec![
                Atom::new(&elements::H, point![0.0, 0.0, 0.0]),
                Atom::new(&elements::HE, point![1.0, 2.0, 3.0]),
            ],
            Some(Units::Bohr),
            Some(Units::Bohr),
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
            Some(Units::Bohr),
            Some(Units::Bohr),
        );

        geometry.rotate(Rotation3::from_axis_angle(
            &Vector3::z_axis(),
            std::f64::consts::FRAC_PI_2,
        ));

        assert_point_close(geometry.atoms[0].position, point![0.0, 1.0, 0.0]);
    }

    #[test]
    fn test_transform_geometry_with_isometry() {
        let mut geometry = Geometry::new(
            "Transform".to_string(),
            vec![Atom::new(&elements::H, point![1.0, 0.0, 0.0])],
            Some(Units::Bohr),
            Some(Units::Bohr),
        );
        let rotation = Rotation3::from_axis_angle(&Vector3::z_axis(), std::f64::consts::FRAC_PI_2);
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
            Some(Units::Bohr),
            Some(Units::Bohr),
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
            Some(Units::Bohr),
            Some(Units::Bohr),
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
            Some(Units::Bohr),
            Some(Units::Bohr),
        );

        geometry.charge_centering();

        assert_point_close(geometry.charge_center(), Point3::origin());
        assert_point_close(geometry.atoms[0].position, point![-4.0 / 3.0, 0.0, 0.0]);
        assert_point_close(geometry.atoms[1].position, point![2.0 / 3.0, 0.0, 0.0]);
    }

    #[test]
    fn test_nucl_repulsion_h2() {
        // Create a simple geometry for the H2 molecule
        let atom1 = Atom::new(&elements::H, point![0.0, 0.0, 0.0]);
        let atom2 = Atom::new(&elements::H, point![0.0, 0.0, 1.40]);

        let geometry = Geometry::new(
            "Hydrogen molecule (H2)".to_string(),
            vec![atom1, atom2],
            Some(Units::Angstrom),
            Some(Units::Bohr),
        );

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
            Some(Units::Bohr),
            Some(Units::Bohr),
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
            Some(Units::Bohr),
            Some(Units::Bohr),
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
