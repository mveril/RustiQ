use nalgebra::Point3;
use periodic_table::Element;

#[derive(Debug, Clone)]
pub struct Atom {
    pub element: &'static Element,
    pub position: Point3<f64>,
}

impl Atom {
    pub(crate) fn new(element: &'static Element, position: Point3<f64>) -> Self {
        Atom { element, position }
    }
}

impl From<&'static Element> for Atom {
    fn from(element: &'static Element) -> Self {
        Atom::new(element, Point3::origin())
    }
}
