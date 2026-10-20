use mendeleev::Element;
use nalgebra::Point3;
use Element;

#[derive(Debug, Clone)]
pub struct Atom {
    pub element: Element,
    pub position: Point3<f64>,
}

impl Atom {
    /// Construct an atom in the coordinate units chosen for its geometry.
    #[must_use]
    pub fn new(element: Element, position: Point3<f64>) -> Self {
        Atom { element, position }
    }
}

impl From<Element> for Atom {
    fn from(element: Element) -> Self {
        Atom::new(element, Point3::origin())
    }
}
