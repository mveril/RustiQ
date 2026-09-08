use std::{
    io::{self, Write},
    time::Duration,
};

use super::{mp2_report::Mp2Reporter, scf_report::ScfReporter};
use rustiq_core::{
    basis::gaussian::basis::Basis,
    calculation::{CalculationObserver, HfCalculationResult},
    config::{HfConfig, ResolvedHfMethod},
    hf::{
        scf_iteration::ScfIteration,
        scf_observer::{ScfObserver, ScfSetupStep},
    },
    mp2::Mp2Result,
};

/// Render scientific notifications; calculation ordering belongs to the core.
pub(crate) struct CalculationReporter<W> {
    scf: ScfReporter<W>,
    enabled: bool,
    show_scf: bool,
    error: Option<io::Error>,
}

impl<W: Write> CalculationReporter<W> {
    pub(crate) fn new(writer: W, enabled: bool, show_scf: bool) -> Self {
        Self {
            scf: ScfReporter::new(writer),
            enabled,
            show_scf,
            error: None,
        }
    }

    pub(crate) fn take_error(&mut self) -> Option<io::Error> {
        self.error.take().or_else(|| self.scf.take_error())
    }

    fn report(&mut self, write: impl FnOnce(&mut ScfReporter<W>) -> io::Result<()>) {
        if self.enabled && self.error.is_none() {
            self.error = write(&mut self.scf).err();
        }
    }
}

impl<W: Write> ScfObserver for CalculationReporter<W> {
    fn on_iteration(&mut self, iteration: &ScfIteration) {
        if self.enabled && self.show_scf && self.error.is_none() {
            self.scf.on_iteration(iteration);
        }
    }
}

impl<W: Write> CalculationObserver for CalculationReporter<W> {
    fn on_basis_start(&mut self) {
        self.report(|scf| writeln!(scf.writer_mut(), "Constructing basis functions..."));
    }

    fn on_basis_ready(&mut self, basis: &Basis, elapsed: Duration) {
        self.report(|scf| {
            writeln!(
                scf.writer_mut(),
                "Constructed {} basis functions in {}",
                basis.nbasis(),
                humantime::format_duration(elapsed)
            )
        });
    }

    fn on_hf_start(&mut self, method: ResolvedHfMethod, config: &HfConfig) {
        self.report(|scf| {
            let writer = scf.writer_mut();
            writeln!(writer, "Conv {}", config.convergence_threshold.into_inner())?;
            writeln!(writer, "Max iter: {}", config.max_iterations.get())?;
            writeln!(writer, "Preparing SCF calculation...")?;
            writeln!(writer, "Resolved HF method: {method}")
        });
    }

    fn on_scf_setup_step(&mut self, step: ScfSetupStep) {
        self.report(|scf| {
            let writer = scf.writer_mut();
            match step {
                ScfSetupStep::CoreHamiltonian => {
                    writeln!(writer, "  Building one-electron core Hamiltonian...")
                }
                ScfSetupStep::OverlapMatrix => writeln!(writer, "  Building overlap matrix..."),
                ScfSetupStep::OverlapOrthogonalizer => {
                    writeln!(writer, "  Building overlap orthogonalizer...")
                }
                ScfSetupStep::OverlapOrthogonalized(info) => writeln!(
                    writer,
                    "  Overlap effective rank: {}/{} ({} discarded, relative threshold {:.3e})...",
                    info.effective_rank,
                    info.basis_dimension,
                    info.discarded_directions,
                    info.relative_threshold,
                ),
                ScfSetupStep::ElectronRepulsionIntegrals => {
                    writeln!(writer, "  Building electron repulsion integrals...")
                }
                ScfSetupStep::InitialDensityGuess => {
                    writeln!(writer, "  Building initial density guess...")
                }
            }
        });
    }

    fn on_hf_complete(&mut self, result: &HfCalculationResult) {
        if self.show_scf {
            self.report(|scf| scf.write_summary(&result.scf));
        }
    }

    fn on_mp2_complete(&mut self, hf: &HfCalculationResult, result: &Mp2Result) {
        self.report(|scf| {
            let label = match hf.method {
                ResolvedHfMethod::Rhf => "RHF MP2",
                ResolvedHfMethod::Uhf => "UHF MP2",
            };
            Mp2Reporter::new(scf.writer_mut(), label).write_summary(result, &hf.scf)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingWriter;
    impl Write for FailingWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed output"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn reporter_retains_output_errors_for_the_cli() {
        let mut reporter = CalculationReporter::new(FailingWriter, true, true);
        reporter.on_basis_start();
        assert_eq!(
            reporter.take_error().unwrap().kind(),
            io::ErrorKind::BrokenPipe
        );
    }

    #[test]
    fn json_mode_disables_all_progress_writes() {
        let mut reporter = CalculationReporter::new(FailingWriter, false, true);
        reporter.on_basis_start();
        reporter.on_hf_start(ResolvedHfMethod::Rhf, &HfConfig::default());
        reporter.on_scf_setup_step(ScfSetupStep::OverlapMatrix);
        assert!(reporter.take_error().is_none());
    }
}
