use std::io::{self, Write};

use crate::hf::{scf_iteration::ScfIteration, scf_observer::ScfObserver, scf_result::ScfResult};

pub(crate) struct ScfReporter<W> {
    writer: W,
    header_written: bool,
}

impl<W> ScfReporter<W>
where
    W: Write,
{
    pub(crate) fn new(writer: W) -> Self {
        Self {
            writer,
            header_written: false,
        }
    }

    pub(crate) fn write_summary(&mut self, result: &ScfResult) -> io::Result<()> {
        if result.converged {
            writeln!(
                self.writer,
                "SCF converged after {} iterations.",
                result.iterations
            )?;
        } else {
            writeln!(
                self.writer,
                "SCF did not converge after {} iterations.",
                result.iterations
            )?;
        }
        writeln!(
            self.writer,
            "SCF delta energy: {:.6e} Hartree",
            result.delta_energy
        )?;
        writeln!(
            self.writer,
            "SCF residual norm: {:.6e}",
            result.residual_norm
        )?;
        writeln!(
            self.writer,
            "Total SCF Energy (without nuclear repulsion): {:.6} Hartree",
            result.electronic_energy
        )?;
        writeln!(
            self.writer,
            "Nuclear Repulsion Energy: {:.6} Hartree",
            result.nuclear_repulsion_energy
        )?;
        writeln!(
            self.writer,
            "Total Energy (including nuclear repulsion): {:.6} Hartree",
            result.total_energy
        )?;
        writeln!(self.writer, "Energy Details:")?;
        writeln!(
            self.writer,
            "  Kinetic Energy: {:.6} Hartree",
            result.energy_details.kinetic_energy
        )?;
        writeln!(
            self.writer,
            "  Nuclear Attraction Energy: {:.6} Hartree",
            result.energy_details.nuclear_attraction_energy
        )?;
        writeln!(
            self.writer,
            "  Electron Repulsion Energy: {:.6} Hartree",
            result.energy_details.electron_repulsion_energy
        )?;
        writeln!(
            self.writer,
            "  Total SCF Energy (without nuclear repulsion): {:.6} Hartree",
            result.electronic_energy
        )?;
        Ok(())
    }

    fn write_header(&mut self) -> io::Result<()> {
        if !self.header_written {
            writeln!(
                self.writer,
                "{:>4} {:>18} {:>14} {:>14}",
                "iter", "E_elec", "delta_E", "residual"
            )?;
            self.header_written = true;
        }
        Ok(())
    }
}

impl<W> ScfObserver for ScfReporter<W>
where
    W: Write,
{
    fn on_iteration(&mut self, iteration: &ScfIteration) {
        self.write_header()
            .expect("failed to write SCF iteration table header");
        writeln!(
            self.writer,
            "{:>4} {:>18.10} {:>14.6e} {:>14.6e}",
            iteration.iteration,
            iteration.electronic_energy,
            iteration.delta_energy,
            iteration.residual_norm
        )
        .expect("failed to write SCF iteration row");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scf_reporter_writes_iteration_and_summary() {
        let mut output = Vec::new();
        {
            let mut reporter = ScfReporter::new(&mut output);
            reporter.on_iteration(&ScfIteration {
                iteration: 1,
                electronic_energy: -1.0,
                delta_energy: 1.0,
                residual_norm: 0.1,
            });
            reporter
                .write_summary(&ScfResult {
                    converged: true,
                    iterations: 1,
                    electronic_energy: -1.0,
                    nuclear_repulsion_energy: 0.2,
                    total_energy: -0.8,
                    delta_energy: 1.0,
                    residual_norm: 0.1,
                    energy_details: crate::hf::scf_energy_details::ScfEnergyDetails {
                        kinetic_energy: 0.3,
                        nuclear_attraction_energy: -1.5,
                        electron_repulsion_energy: 0.2,
                    },
                })
                .unwrap();
        }

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("iter"));
        assert!(output.contains("SCF converged after 1 iterations."));
        assert!(output.contains("Energy Details:"));
    }
}
