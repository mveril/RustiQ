use crate::molecule::PyMolecule;
use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
};
use rustiq_core::{
    basis::BasisFile,
    calculation::{CalculationBuilder, HfOutcome},
    config::{
        validated::{DiisSize, PositiveFiniteF64},
        HfConfig, HfMethod, MoleculeConfig, Mp2Config,
    },
    molecules::geometry::Geometry,
};
use std::{
    fs::File,
    num::{NonZeroU8, NonZeroUsize},
    path::PathBuf,
    sync::Arc,
};

fn value_error(error: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(error.to_string())
}

#[pyclass(name = "BasisSet", module = "rustiq", frozen)]
pub struct PyBasisSet {
    inner: Arc<BasisFile>,
}

#[pymethods]
impl PyBasisSet {
    #[staticmethod]
    fn from_file(path: PathBuf) -> PyResult<Self> {
        Ok(Self {
            inner: Arc::new(BasisFile::from_reader(File::open(path)?).map_err(value_error)?),
        })
    }

    #[staticmethod]
    fn from_json(data: &str) -> PyResult<Self> {
        Ok(Self {
            inner: Arc::new(BasisFile::from_reader(data.as_bytes()).map_err(value_error)?),
        })
    }

    #[getter]
    fn name(&self) -> &str {
        self.inner.name()
    }
}

struct Calculation {
    geometry: Geometry,
    molecule: MoleculeConfig,
    basis: Arc<BasisFile>,
    hf: HfConfig,
}

impl Calculation {
    fn new(
        molecule: &PyMolecule,
        basis: &PyBasisSet,
        method: HfMethod,
        max_iterations: usize,
        convergence_threshold: f64,
        diis: bool,
        diis_size: usize,
    ) -> PyResult<Self> {
        let geometry = molecule
            .geometry
            .read()
            .map_err(|_| PyRuntimeError::new_err("geometry lock poisoned"))?
            .clone();
        if geometry
            .atoms
            .iter()
            .any(|a| a.position.iter().any(|v| !v.is_finite()))
        {
            return Err(value_error("coordinates must be finite"));
        }
        let hf = HfConfig {
            method: method.into(),
            max_iterations: NonZeroUsize::new(max_iterations)
                .ok_or_else(|| value_error("max_iterations must be positive"))?,
            convergence_threshold: PositiveFiniteF64::try_new(convergence_threshold)
                .map_err(value_error)?,
            diis,
            diis_size: DiisSize::try_new(diis_size).map_err(value_error)?,
            ..Default::default()
        };
        let config = MoleculeConfig {
            units: molecule.unit,
            charge: molecule.charge.into(),
            multiplicity: NonZeroU8::new(molecule.multiplicity).unwrap().into(),
        };
        hf.resolve_method(&config.build(geometry.clone()).map_err(value_error)?)
            .map_err(value_error)?;
        Ok(Self {
            geometry,
            molecule: config,
            basis: Arc::clone(&basis.inner),
            hf,
        })
    }

    fn run(&self, py: Python<'_>) -> PyResult<PyHfResult> {
        py.detach(|| {
            let prepared = CalculationBuilder::new(&self.geometry, &self.basis)
                .with_molecule_config(self.molecule)
                .with_hf(self.hf.clone())
                .prepare()
                .map_err(value_error)?;
            let inner = prepared
                .run_hf()
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            Ok(PyHfResult { inner })
        })
    }
}

macro_rules! hf_class {
    ($rust:ident, $python:literal, $method:ident) => {
        #[pyclass(name = $python, module = "rustiq", frozen)]
        pub struct $rust { inner: Calculation }
        #[pymethods]
        impl $rust {
            #[new]
            #[pyo3(signature = (molecule, basis, *, max_iterations=100, convergence_threshold=1e-8, diis=true, diis_size=6))]
            fn new(molecule: &PyMolecule, basis: &PyBasisSet, max_iterations: usize,
                convergence_threshold: f64, diis: bool, diis_size: usize) -> PyResult<Self> {
                Ok(Self { inner: Calculation::new(molecule, basis, HfMethod::$method,
                    max_iterations, convergence_threshold, diis, diis_size)? })
            }
            fn run(&self, py: Python<'_>) -> PyResult<PyHfResult> { self.inner.run(py) }
        }
    }
}
hf_class!(PyRhf, "RHF", Rhf);
hf_class!(PyUhf, "UHF", Uhf);

#[pyclass(name = "HfResult", module = "rustiq", frozen)]
pub struct PyHfResult {
    inner: HfOutcome,
}

#[pymethods]
impl PyHfResult {
    #[getter]
    fn converged(&self) -> bool {
        self.inner.is_converged()
    }
    #[getter]
    fn method(&self) -> String {
        self.inner.method().to_string()
    }
    #[getter]
    fn total_energy(&self) -> f64 {
        self.inner.summary().scf.total_energy
    }
    #[getter]
    fn electronic_energy(&self) -> f64 {
        self.inner.summary().scf.electronic_energy
    }
    #[getter]
    fn nuclear_repulsion_energy(&self) -> f64 {
        self.inner.summary().scf.nuclear_repulsion_energy
    }
    #[getter]
    fn iterations(&self) -> usize {
        self.inner.summary().scf.iterations
    }
    #[getter]
    fn residual_norm(&self) -> f64 {
        self.inner.summary().scf.residual_norm
    }
    #[getter]
    fn s_squared(&self) -> Option<f64> {
        self.inner.summary().scf.spin.map(|s| s.s_squared)
    }
    #[getter]
    fn spin_contamination(&self) -> Option<f64> {
        self.inner.summary().scf.spin.map(|s| s.spin_contamination)
    }
    #[pyo3(signature = (*, frozen_orbitals=0))]
    fn mp2(&self, py: Python<'_>, frozen_orbitals: usize) -> PyResult<PyMp2Result> {
        let HfOutcome::Converged(hf) = &self.inner else {
            return Err(PyRuntimeError::new_err(
                "MP2 requires converged HF orbitals",
            ));
        };
        py.detach(|| {
            let result = hf
                .mp2(Mp2Config {
                    frozen_orbitals: frozen_orbitals.into(),
                })
                .map_err(value_error)?;
            Ok(PyMp2Result {
                correlation_energy: result.correlation_energy,
                electronic_energy: result.electronic_energy,
                total_energy: result.electronic_energy + self.nuclear_repulsion_energy(),
                frozen_orbitals,
            })
        })
    }
}

#[pyclass(name = "Mp2Result", module = "rustiq", frozen, get_all)]
pub struct PyMp2Result {
    correlation_energy: f64,
    electronic_energy: f64,
    total_energy: f64,
    frozen_orbitals: usize,
}
