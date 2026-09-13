use pyo3::prelude::*;

#[pymodule]
#[pyo3(name = "rustiq")]
fn rustiq_python(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
