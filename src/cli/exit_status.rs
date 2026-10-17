//! Process status policy belongs to the CLI, independently of scientific outcomes.
use std::process::ExitCode;

use miette::{Diagnostic, Report};
use rustiq_core::calculation::CalculationError;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
#[error("HF did not converge")]
pub(crate) struct NonConvergence;

#[derive(Debug, Error, Diagnostic)]
#[error("batch contains {failed} failed and {non_converged} non-converged calculations")]
pub(crate) struct IncompleteBatch {
    pub succeeded: usize,
    pub non_converged: usize,
    pub failed: usize,
    pub general_errors: usize,
}

pub(crate) fn is_non_convergence(error: &Report) -> bool {
    error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<CalculationError>(),
            Some(CalculationError::HfNotConverged { .. })
        )
    })
}

pub(crate) fn for_error(error: &Report) -> ExitCode {
    if let Some(batch) = error.downcast_ref::<IncompleteBatch>() {
        if batch.succeeded > 0 {
            return ExitCode::from(4);
        }
        return if batch.general_errors == 0 {
            ExitCode::from(3)
        } else {
            ExitCode::FAILURE
        };
    }
    if error.is::<NonConvergence>() || is_non_convergence(error) {
        ExitCode::from(3)
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustiq_core::calculation::CalculationExecutionError;

    #[test]
    fn incomplete_batch_codes_cover_all_outcome_combinations() {
        // (successful, non-converged results, execution errors, general errors, code)
        for (succeeded, non_converged, failed, general_errors, expected) in [
            (0, 1, 0, 0, 3),
            (0, 0, 1, 0, 3),
            (0, 1, 1, 0, 3),
            (0, 0, 1, 1, 1),
            (0, 1, 1, 1, 1),
            (0, 0, 2, 1, 1),
            (1, 1, 0, 0, 4),
            (1, 0, 1, 0, 4),
            (1, 0, 1, 1, 4),
            (1, 1, 2, 1, 4),
        ] {
            let error = IncompleteBatch {
                succeeded,
                non_converged,
                failed,
                general_errors,
            }
            .into();
            assert_eq!(for_error(&error), ExitCode::from(expected));
        }
    }

    #[test]
    fn non_convergence_survives_source_and_context_wrappers() {
        let error =
            CalculationExecutionError::from(CalculationError::HfNotConverged { iterations: 1 });
        let report = Report::new(error)
            .with_source_code(miette::NamedSource::new("calculation.toml", "[method.mp2]"))
            .wrap_err("calculation failed");
        assert!(is_non_convergence(&report));
        assert_eq!(for_error(&report), ExitCode::from(3));
    }

    #[test]
    fn standalone_non_convergence_and_general_errors_have_distinct_codes() {
        assert_eq!(for_error(&NonConvergence.into()), ExitCode::from(3));
        let error = miette::miette!("failed to write calculation report");
        assert!(!is_non_convergence(&error));
        assert_eq!(for_error(&error), ExitCode::FAILURE);
    }
}
