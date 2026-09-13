use nalgebra::Point3;
use pyo3::prelude::*;
use rustiq_core::molecules::atom::Atom;

use crate::geometry::element_like::ElementLike;

#[pyclass(name = "Atom")]
pub struct PyAtom {
    pub(crate) inner: Atom,
}

impl PyAtom {
    pub(crate) fn build(element: ElementLike, x: f64, y: f64, z: f64) -> PyResult<Atom> {
        Ok(Atom::new(element.resolve()?, Point3::new(x, y, z)))
    }
}

#[pymethods]
impl PyAtom {
    #[new]
    fn new(element: ElementLike, x: f64, y: f64, z: f64) -> PyResult<Self> {
        Ok(Self {
            inner: Self::build(element, x, y, z)?,
        })
    }

    #[getter]
    fn element(&self) -> &str {
        self.inner.element.symbol
    }

    #[getter]
    fn coordinates(&self) -> (f64, f64, f64) {
        let position = self.inner.position;
        (position.x, position.y, position.z)
    }
}
