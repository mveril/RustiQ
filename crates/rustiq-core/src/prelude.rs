//! Common types for direct calculations and portable `.rustiq` workflows.
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
//!
//! A portable archive can also supply reusable artifacts to normal execution:
//!
//! ```no_run
//! use rustiq_core::prelude::*;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let geometry = Geometry::from_reader(std::io::Cursor::new(
//!     "2\nH2\nH 0 0 -0.37\nH 0 0 0.37\n",
//! ))?;
//! let basis = BasisFile::from_reader(std::fs::File::open("sto-3g.json")?)?;
//! let prepared = CalculationBuilder::new(&geometry, &basis).prepare()?;
//! let hf = prepared.run_hf()?;
//! let eri: &CompactEri = hf.ao_eri();
//!
//! let directory = tempfile::tempdir()?;
//! let path = directory.path().join("water.rustiq");
//! let mut data = RustiQData::from_calculation(&prepared)?;
//! data.write_with_eri(&path, eri)?;
//!
//! let restored = RustiQData::open(&path)?.prepare_calculation()?;
//! let result = restored.execute()?;
//!
//! // Strict ownership-transfer path: missing or incompatible ERIs are errors.
//! let mut data = RustiQData::open(&path)?;
//! let eri = data.take_compatible_eri(&prepared)?;
//! let result = prepared.execute_with_eri(eri)?;
//! let _ = (result, Option::<RustiQBundle>::None);
//! # Ok(())
//! # }
//! ```

pub use crate::{
    basis::BasisFile,
    calculation::{
        ArtifactReuseDecision, ArtifactReuseEvent, CalculationBuilder, CalculationEvent,
        CalculationExecution, CalculationExecutionError, CalculationResult, Converged, HfOutcome,
        HfSolution, PreparedCalculation, Unconverged,
    },
    config::{HfConfig, HfMethod, MoleculeConfig, Mp2Config},
    molecules::{atom::Atom, geometry::Geometry, units::Units},
    persistence::{CompactEri, RustiQBundle, RustiQData},
};
