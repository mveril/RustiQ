use pyo3::{exceptions::PyValueError, prelude::*, types::PyAny};
use rustiq_core::molecules::{molecule::Molecule, units::Units};
use std::{
    num::NonZeroU8,
    sync::{Arc, RwLock},
};

use crate::geometry::{atom_from_python, PyGeometry};

#[pyclass(name = "Molecule")]
pub struct PyMolecule {
    pub(crate) geometry: Arc<RwLock<rustiq_core::molecules::geometry::Geometry>>,
    pub(crate) charge: i32,
    pub(crate) multiplicity: u8,
    pub(crate) unit: Units,
}

#[pymethods]
impl PyMolecule {
    #[new]
    #[pyo3(signature = (atoms, charge = 0, multiplicity = 1, comment = "", *, units = "bohr"))]
    fn new(
        atoms: &Bound<'_, PyAny>,
        charge: i32,
        multiplicity: u8,
        comment: &str,
        units: &str,
    ) -> PyResult<Self> {
        let atoms = atoms
            .try_iter()?
            .map(|value| atom_from_python(&value?))
            .collect::<PyResult<Vec<_>>>()?;
        let geometry = rustiq_core::molecules::geometry::Geometry::new(comment.to_owned(), atoms);
        let multiplicity = NonZeroU8::new(multiplicity)
            .ok_or_else(|| PyValueError::new_err("multiplicity must be greater than zero"))?;

        let unit = match units {
            "bohr" => Units::Bohr,
            "angstrom" => Units::Angstrom,
            _ => return Err(PyValueError::new_err("units must be 'bohr' or 'angstrom'")),
        };
        Molecule::try_new(geometry.clone(), unit, charge, multiplicity)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;

        Ok(Self {
            geometry: Arc::new(RwLock::new(geometry)),
            charge,
            multiplicity: multiplicity.get(),
            unit,
        })
    }

    #[getter]
    fn geometry(&self) -> PyGeometry {
        PyGeometry {
            inner: Arc::clone(&self.geometry),
        }
    }

    #[getter]
    fn charge(&self) -> i32 {
        self.charge
    }

    #[getter]
    fn multiplicity(&self) -> u8 {
        self.multiplicity
    }

    #[getter]
    fn units(&self) -> &'static str {
        match self.unit {
            Units::Bohr => "bohr",
            Units::Angstrom => "angstrom",
        }
    }
}
