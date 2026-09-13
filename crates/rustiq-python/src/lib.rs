use pyo3::prelude::*;
mod calculation;
mod geometry;
mod molecule;
use geometry::{PyAtom, PyGeometry};
use molecule::PyMolecule;

#[pymodule]
#[pyo3(name = "rustiq")]
fn rustiq_python(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    module.add_class::<PyAtom>()?;
    module.add_class::<PyGeometry>()?;
    module.add_class::<PyMolecule>()?;
    module.add_class::<calculation::PyBasisSet>()?;
    module.add_class::<calculation::PyRhf>()?;
    module.add_class::<calculation::PyUhf>()?;
    module.add_class::<calculation::PyHfResult>()?;
    module.add_class::<calculation::PyMp2Result>()?;
    Ok(())
}
