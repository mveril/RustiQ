use std::io::{self, Write};

use crate::{hf::scf_result::ScfResult, mp2::Mp2Result};

pub(crate) struct Mp2Reporter<W> {
    writer: W,
    label: &'static str,
}

impl<W> Mp2Reporter<W>
where
    W: Write,
{
    pub(crate) fn new(writer: W, label: &'static str) -> Self {
        Self { writer, label }
    }

    pub(crate) fn write_summary(
        &mut self,
        result: &Mp2Result,
        scf_result: &ScfResult,
    ) -> io::Result<()> {
        let total_energy = result.electronic_energy + scf_result.nuclear_repulsion_energy;
        writeln!(
            self.writer,
            "{} correlation energy: {:.6} Hartree",
            self.label, result.correlation_energy
        )?;
        writeln!(
            self.writer,
            "{} total energy (without nuclear repulsion): {:.6} Hartree",
            self.label, result.electronic_energy
        )?;
        writeln!(
            self.writer,
            "{} total energy (including nuclear repulsion): {:.6} Hartree",
            self.label, total_energy
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mp2_reporter_writes_summary() {
        let mut output = Vec::new();
        {
            let mut reporter = Mp2Reporter::new(&mut output, "RHF MP2");
            reporter
                .write_summary(
                    &Mp2Result {
                        correlation_energy: -0.1,
                        electronic_energy: -1.2,
                    },
                    &ScfResult {
                        converged: true,
                        iterations: 1,
                        electronic_energy: -1.1,
                        nuclear_repulsion_energy: 0.3,
                        total_energy: -0.8,
                        delta_energy: 0.0,
                        residual_norm: 0.0,
                        energy_details: crate::hf::scf_energy_details::ScfEnergyDetails {
                            kinetic_energy: 0.0,
                            nuclear_attraction_energy: 0.0,
                            electron_repulsion_energy: 0.0,
                        },
                        timings: crate::hf::scf_result::ScfTimings::default(),
                    },
                )
                .unwrap();
        }

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("RHF MP2 correlation energy"));
        assert!(output.contains("RHF MP2 total energy (including nuclear repulsion)"));
    }
}
