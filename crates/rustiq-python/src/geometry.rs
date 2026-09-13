use nalgebra::{Rotation3, Translation3, Unit, Vector3};
use pyo3::{
    exceptions::PyValueError,
    prelude::*,
    types::{PyAny, PyTuple},
};
use rustiq_core::molecules::geometry::Geometry;
use std::sync::{Arc, RwLock};
mod atom;
mod element_like;
pub use atom::PyAtom;

fn vector_from_python(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Vector3<f64>> {
    let tuple = value
        .cast::<PyTuple>()
        .map_err(|_| PyValueError::new_err(format!("{name} must be a tuple of three numbers")))?;
    let (x, y, z): (f64, f64, f64) = tuple
        .extract()
        .map_err(|_| PyValueError::new_err(format!("{name} must be a tuple of three numbers")))?;
    Ok(Vector3::new(x, y, z))
}

pub(crate) fn atom_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<rustiq_core::molecules::atom::Atom> {
    if let Ok(atom) = value.extract::<PyRef<'_, PyAtom>>() {
        return Ok(atom.inner.clone());
    }

    let (element, x, y, z) = value.extract()?;
    PyAtom::build(element, x, y, z)
}

#[pyclass(name = "Geometry")]
pub struct PyGeometry {
    pub(crate) inner: Arc<RwLock<Geometry>>,
}

#[pymethods]
impl PyGeometry {
    #[new]
    #[pyo3(signature = (atoms, comment = ""))]
    fn new(atoms: &Bound<'_, PyAny>, comment: &str) -> PyResult<Self> {
        let atoms = atoms
            .try_iter()?
            .map(|value| atom_from_python(&value?))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            inner: Arc::new(RwLock::new(Geometry::new(comment.to_owned(), atoms))),
        })
    }

    #[getter]
    fn comment(&self) -> PyResult<String> {
        Ok(self
            .inner
            .read()
            .map_err(|_| PyValueError::new_err("geometry lock poisoned"))?
            .comment
            .clone())
    }

    #[setter]
    fn set_comment(&self, comment: String) -> PyResult<()> {
        self.inner
            .write()
            .map_err(|_| PyValueError::new_err("geometry lock poisoned"))?
            .comment = comment;
        Ok(())
    }

    fn __len__(&self) -> PyResult<usize> {
        Ok(self
            .inner
            .read()
            .map_err(|_| PyValueError::new_err("geometry lock poisoned"))?
            .atoms
            .len())
    }

    fn translate(&self, translation: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner
            .write()
            .map_err(|_| PyValueError::new_err("geometry lock poisoned"))?
            .translate(Translation3::from(vector_from_python(
                translation,
                "translation",
            )?));
        Ok(())
    }

    fn rotate(&self, axis: &Bound<'_, PyAny>, angle: f64) -> PyResult<()> {
        let axis = Unit::try_new(vector_from_python(axis, "axis")?, f64::EPSILON)
            .ok_or_else(|| PyValueError::new_err("axis must not be the zero vector"))?;
        self.inner
            .write()
            .map_err(|_| PyValueError::new_err("geometry lock poisoned"))?
            .rotate(Rotation3::from_axis_angle(&axis, angle));
        Ok(())
    }

    fn centering(&self) -> PyResult<()> {
        self.inner
            .write()
            .map_err(|_| PyValueError::new_err("geometry lock poisoned"))?
            .centering();
        Ok(())
    }

    fn mass_centering(&self) -> PyResult<()> {
        self.inner
            .write()
            .map_err(|_| PyValueError::new_err("geometry lock poisoned"))?
            .mass_centering()
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    fn charge_centering(&self) -> PyResult<()> {
        self.inner
            .write()
            .map_err(|_| PyValueError::new_err("geometry lock poisoned"))?
            .charge_centering();
        Ok(())
    }

    fn orient_along_principal_axes(&self) -> PyResult<()> {
        self.inner
            .write()
            .map_err(|_| PyValueError::new_err("geometry lock poisoned"))?
            .orient_along_principal_axes()
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    fn coordinates(&self) -> PyResult<Vec<(f64, f64, f64)>> {
        Ok(self
            .inner
            .read()
            .map_err(|_| PyValueError::new_err("geometry lock poisoned"))?
            .atoms
            .iter()
            .map(|atom| (atom.position.x, atom.position.y, atom.position.z))
            .collect())
    }
}
