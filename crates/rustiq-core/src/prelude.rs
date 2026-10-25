//! Common inputs, configuration, execution, and results for direct calculations.
//!
//! Import this module to use [`CalculationExecution`] methods on builders and
//! prepared calculations. Advanced storage and raw-integral workflows use
//! explicit imports from [`crate::basis`] and [`crate::persistence`].
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
//! let hf = result.hf.into_converged()?;
//! let mp2 = hf.mp2(Mp2Config::default())?;
//! println!("MP2 correlation: {}", mp2.correlation_energy);
//! # Ok(())
//! # }
//! ```
//!
pub use crate::{
    basis::BasisFile,
    calculation::{
        AnyHfSolution, CalculationBuilder, CalculationError, CalculationEvent,
        CalculationExecution, CalculationExecutionError, CalculationResult, Converged, HfModel,
        HfOutcome, HfReference, HfSolution, MoCoefficients, Occupations, OrbitalEnergies,
        PreparedCalculation, Restricted, RestrictedMethod, RhfSolution, ScfMethod, ScfSolution,
        Spin, UhfSolution, Unconverged, Unrestricted, UnrestrictedMethod,
    },
    config::{HfConfig, HfMethod, IntegralConfig, Located, MoleculeConfig, Mp2Config},
    molecules::{Atom, Geometry, GeometryParseError, Units},
    Element,
};
