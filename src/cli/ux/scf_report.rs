use std::io::{self, Write};

use crate::cli::color;

use rustiq_core::calculation::{ScfIteration, ScfResult};

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

    pub(crate) fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Keep related SCF report sections together in their terminal display order"
    )]
    pub(crate) fn write_summary(&mut self, result: &ScfResult, converged: bool) -> io::Result<()> {
        if converged {
            writeln!(
                self.writer,
                "{}",
                color::success(format!(
                    "SCF converged after {} iterations.",
                    result.iterations
                ))
            )?;
        } else {
            writeln!(
                self.writer,
                "{}",
                color::error(format!(
                    "SCF did not converge after {} iterations.",
                    result.iterations
                ))
            )?;
        }
        writeln!(
            self.writer,
            "{} {:.6e} Hartree",
            color::title("SCF delta energy:"),
            result.delta_energy
        )?;
        writeln!(
            self.writer,
            "{} {:.6e}",
            color::title("SCF residual norm:"),
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
            "{} {} Hartree",
            color::title("Total Energy (including nuclear repulsion):"),
            color::value(format!("{:.6}", result.total_energy)),
        )?;
        if let Some(spin) = result.spin {
            let qualifier = if converged {
                ""
            } else {
                " (unconverged orbitals)"
            };
            writeln!(self.writer, "Spin <S^2>{qualifier}: {:.6}", spin.s_squared)?;
            writeln!(self.writer, "Ideal <S^2>: {:.6}", spin.ideal_s_squared)?;
            writeln!(
                self.writer,
                "Spin contamination: {:.6}",
                spin.spin_contamination
            )?;
        }
        writeln!(
            self.writer,
            "Overlap effective rank: {}/{} ({} discarded, relative threshold {:.3e})",
            result.orthogonalization.effective_rank,
            result.orthogonalization.basis_dimension,
            result.orthogonalization.discarded_directions,
            result.orthogonalization.relative_threshold,
        )?;
        writeln!(self.writer, "{}", color::title("Energy Details:"))?;
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
        writeln!(self.writer, "{}", color::title("Timings:"))?;
        writeln!(
            self.writer,
            "  Setup total: {}",
            humantime::format_duration(result.timings.setup.total)
        )?;
        writeln!(
            self.writer,
            "    Core Hamiltonian: {}",
            humantime::format_duration(result.timings.setup.core_hamiltonian)
        )?;
        writeln!(
            self.writer,
            "    Overlap matrix: {}",
            humantime::format_duration(result.timings.setup.overlap)
        )?;
        writeln!(
            self.writer,
            "    Orthogonalizer: {}",
            humantime::format_duration(result.timings.setup.orthogonalizer)
        )?;
        writeln!(
            self.writer,
            "    Electron repulsion integrals: {}",
            humantime::format_duration(result.timings.setup.electron_repulsion_integrals)
        )?;
        writeln!(
            self.writer,
            "    Density guess: {}",
            humantime::format_duration(result.timings.setup.density_guess)
        )?;
        writeln!(
            self.writer,
            "    Initial orbitals: {}",
            humantime::format_duration(result.timings.setup.initial_orbitals)
        )?;
        writeln!(
            self.writer,
            "  SCF iterations: {}",
            humantime::format_duration(result.timings.iterations)
        )?;
        writeln!(
            self.writer,
            "  Final energy details: {}",
            humantime::format_duration(result.timings.final_energy_details)
        )?;
        writeln!(
            self.writer,
            "  Total wall time: {}",
            humantime::format_duration(result.timings.total)
        )?;
        Ok(())
    }

    fn write_header(&mut self) -> io::Result<()> {
        if !self.header_written {
            let header = format!(
                "{:>4} {:>18} {:>14} {:>14}",
                "iter", "E_elec", "delta_E", "residual"
            );
            writeln!(self.writer, "{}", color::title(header))?;
            self.header_written = true;
        }
        Ok(())
    }
    pub(crate) fn write_iteration(&mut self, iteration: &ScfIteration) -> io::Result<()> {
        self.write_header().and_then(|()| {
            writeln!(
                self.writer,
                "{:>4} {:>18.10} {:>14.6e} {:>14.6e}",
                iteration.iteration,
                iteration.electronic_energy,
                iteration.delta_energy,
                iteration.residual_norm
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::ColorChoice;
    use rustiq_core::calculation::{OrthogonalizationInfo, ScfEnergyDetails, ScfTimings};

    #[test]
    fn test_scf_reporter_writes_iteration_and_summary() {
        let mut output = Vec::new();
        {
            let mut reporter = ScfReporter::new(&mut output);
            reporter
                .write_iteration(&ScfIteration {
                    iteration: 1,
                    electronic_energy: -1.0,
                    delta_energy: 1.0,
                    residual_norm: 0.1,
                })
                .unwrap();
            reporter
                .write_summary(
                    &ScfResult {
                        iterations: 1,
                        electronic_energy: -1.0,
                        nuclear_repulsion_energy: 0.2,
                        total_energy: -0.8,
                        delta_energy: 1.0,
                        residual_norm: 0.1,
                        spin: None,
                        energy_details: ScfEnergyDetails {
                            kinetic_energy: 0.3,
                            nuclear_attraction_energy: -1.5,
                            electron_repulsion_energy: 0.2,
                        },
                        orthogonalization: OrthogonalizationInfo::default(),
                        timings: ScfTimings::default(),
                    },
                    true,
                )
                .unwrap();
        }

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("iter"));
        assert!(output.contains("SCF converged after 1 iterations."));
        assert!(output.contains("Energy Details:"));
        assert!(output.contains("Overlap effective rank:"));
        assert!(output.contains("Timings:"));
    }

    #[test]
    fn scf_header_keeps_alignment_when_colored() {
        let render = |mode| {
            color::with_test_color(mode, || {
                let mut output = Vec::new();
                ScfReporter::new(&mut output)
                    .write_iteration(&ScfIteration {
                        iteration: 1,
                        electronic_energy: -1.0,
                        delta_energy: 0.0,
                        residual_norm: 0.0,
                    })
                    .unwrap();
                String::from_utf8(output)
                    .unwrap()
                    .lines()
                    .next()
                    .unwrap()
                    .to_owned()
            })
        };
        let plain = render(ColorChoice::Never);
        let colored = render(ColorChoice::Always);
        let strip_ansi = |text: &str| {
            let mut visible = String::new();
            let mut escape = false;
            for ch in text.chars() {
                if escape {
                    if ch == 'm' {
                        escape = false;
                    }
                } else if ch == '\x1b' {
                    escape = true;
                } else {
                    visible.push(ch);
                }
            }
            visible
        };
        assert_eq!(
            plain,
            "iter             E_elec        delta_E       residual"
        );
        assert!(colored.contains("\x1b["));
        assert_eq!(strip_ansi(&colored), plain);
    }
}
