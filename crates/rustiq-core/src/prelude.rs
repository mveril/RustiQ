//! Common types and the execution trait for direct Rust calculations.
//!
//! ```no_run
//! use rustiq_core::prelude::*;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let geometry = Geometry::from_reader(std::io::Cursor::new(
//!     "2\nH2\nH 0 0 -0.37\nH 0 0 0.37\n",
//! ))?;
//! let basis = BasisFile::from_reader(std::fs::File::open("sto-3g.json")?)?;
//! let prepared = CalculationBuilder::new(&geometry, &basis)
//!     .with_molecule_config(MoleculeConfig { units: Units::Angstrom, ..Default::default() })
//!     .with_mp2(Mp2Config::default())
//!     .prepare()?;
//! let result = prepared.execute()?;
//! println!("HF energy: {}", result.hf.summary().scf.total_energy);
//!
//! // A new MP2 evaluation reuses the same HF orbitals and integrals.
//! let HfOutcome::Converged(hf) = result.hf else { return Err("HF did not converge".into()); };
//! let mp2 = hf.mp2(Mp2Config::default())?;
//! println!("MP2 correlation: {}", mp2.correlation_energy);
//! # Ok(())
//! # }
//! ```

pub use crate::{
    basis::BasisFile,
    calculation::{
        CalculationBuilder, CalculationExecution, CalculationExecutionError, CalculationResult,
        Converged, HfOutcome, HfSolution, PreparedCalculation, Unconverged,
    },
    config::{HfConfig, HfMethod, MoleculeConfig, Mp2Config},
    molecules::{atom::Atom, geometry::Geometry, units::Units},
};
