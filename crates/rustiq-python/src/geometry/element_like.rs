#[derive(FromPyObject)]
pub(crate) enum ElementLike {
    Symbol(String),
    AtomicNumber(u8),
}
use periodic_table::{periodic_table, Element};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

impl ElementLike {
    pub(crate) fn resolve(self) -> PyResult<&'static Element> {
        match self {
            Self::Symbol(symbol) => periodic_table()
                .iter()
                .find(|element| element.symbol == symbol)
                .copied()
                .ok_or_else(|| {
                    PyValueError::new_err(format!("unknown chemical element symbol: {symbol}"))
                }),

            Self::AtomicNumber(atomic_number) => {
                if atomic_number == 0 {
                    return Err(PyValueError::new_err(
                        "atomic number must be greater than zero",
                    ));
                }

                periodic_table()
                    .get(usize::from(atomic_number - 1))
                    .copied()
                    .ok_or_else(|| {
                        PyValueError::new_err(format!("unknown atomic number: {atomic_number}"))
                    })
            }
        }
    }
}
