use std::time::Instant;

use nalgebra::DMatrix;
use thiserror::Error;

use crate::calculation::EriCacheEvent;
use crate::{
    basis::gaussian::basis::Basis,
    config::{
        validated::PositiveFiniteF64, IntegralConfig, OrthogonalizationConfig,
        DEFAULT_ERI_SCHWARZ_THRESHOLD,
    },
    eri::{CompactEri, EriError},
    molecules::molecule::Molecule,
    persistence::EriCache,
};

use super::{
    integrals::{IntegralBuilder, ScfIntegrals},
    numerical_error::NumericalError,
    orthogonalization::{ensure_sufficient_rank, orthogonalizer, OrthogonalizationInfo},
    scf_result::ScfSetupTimings,
};

/// A scientific stage performed while preparing an SCF calculation.
///
/// This deliberately carries no presentation text: frontends choose how to
/// describe these stages to their users.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScfSetupStep {
    CoreHamiltonian,
    OverlapMatrix,
    OverlapOrthogonalizer,
    OverlapOrthogonalized(OrthogonalizationInfo),
    ElectronRepulsionIntegrals,
    InitialDensityGuess,
}

/// Shared SCF setup state, prepared before constructing either SCF engine.
pub(crate) struct PreparedScfSetup {
    pub(crate) integrals: ScfIntegrals,
    pub(crate) orthogonalizer: DMatrix<f64>,
    pub(crate) orthogonalization: OrthogonalizationInfo,
    pub(crate) timings: ScfSetupTimings,
}

#[derive(Debug, Error)]
pub(crate) enum ScfPreparationError {
    #[error(transparent)]
    ElectronRepulsion(#[from] EriError),
    #[error(transparent)]
    Numerical(#[from] NumericalError),
}

pub(crate) fn prepare_scf_setup(
    molecule: &Molecule,
    basis: &Basis,
    required_occupied_orbitals: usize,
    linear_dependency_threshold: f64,
    progress: impl FnMut(ScfSetupStep),
) -> Result<PreparedScfSetup, ScfPreparationError> {
    prepare_scf_setup_with_thresholds(
        molecule,
        basis,
        required_occupied_orbitals,
        linear_dependency_threshold,
        Some(
            PositiveFiniteF64::try_new(DEFAULT_ERI_SCHWARZ_THRESHOLD)
                .expect("default ERI Schwarz threshold is valid"),
        ),
        None,
        None,
        progress,
        |_| {},
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "Scientific setup requires these distinct inputs; preserve the existing interface"
)]
pub(crate) fn prepare_configured_scf_setup(
    molecule: &Molecule,
    basis: &Basis,
    required_occupied_orbitals: usize,
    orthogonalization: &OrthogonalizationConfig,
    integrals: &IntegralConfig,
    eri_cache: Option<&EriCache>,
    supplied_eri: Option<CompactEri>,
    progress: impl FnMut(ScfSetupStep),
    cache_event: impl FnMut(EriCacheEvent),
) -> Result<PreparedScfSetup, ScfPreparationError> {
    prepare_scf_setup_with_thresholds(
        molecule,
        basis,
        required_occupied_orbitals,
        orthogonalization
            .linear_dependency_threshold
            .value
            .into_inner(),
        integrals.schwarz_threshold.value,
        eri_cache,
        supplied_eri,
        progress,
        cache_event,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "Scientific setup requires these distinct inputs; preserve the existing interface"
)]
fn prepare_scf_setup_with_thresholds(
    molecule: &Molecule,
    basis: &Basis,
    required_occupied_orbitals: usize,
    linear_dependency_threshold: f64,
    eri_schwarz_threshold: Option<PositiveFiniteF64>,
    eri_cache: Option<&EriCache>,
    supplied_eri: Option<CompactEri>,
    mut progress: impl FnMut(ScfSetupStep),
    mut cache_event: impl FnMut(EriCacheEvent),
) -> Result<PreparedScfSetup, ScfPreparationError> {
    let setup_start = Instant::now();
    let mut timings = ScfSetupTimings::default();
    let builder = IntegralBuilder::new(molecule, basis, eri_schwarz_threshold, eri_cache);

    progress(ScfSetupStep::CoreHamiltonian);
    let step_start = Instant::now();
    let (kinetic, nuclear_attraction) = builder.core_hamiltonian();
    timings.core_hamiltonian = step_start.elapsed();

    progress(ScfSetupStep::OverlapMatrix);
    let step_start = Instant::now();
    let overlap = builder.overlap();
    timings.overlap = step_start.elapsed();
    crate::debug_assert_is_symmetric!(&overlap, 1e-8);

    progress(ScfSetupStep::OverlapOrthogonalizer);
    let step_start = Instant::now();
    let orthogonalization_result =
        orthogonalizer(&overlap, "overlap", linear_dependency_threshold)?;
    let orthogonalization = orthogonalization_result.info;
    ensure_sufficient_rank(orthogonalization, required_occupied_orbitals)?;
    let orthogonalizer = orthogonalization_result.matrix;
    timings.orthogonalizer = step_start.elapsed();
    progress(ScfSetupStep::OverlapOrthogonalized(orthogonalization));

    progress(ScfSetupStep::ElectronRepulsionIntegrals);
    let step_start = Instant::now();
    let (electron_repulsion, eri_cache_event) = match supplied_eri {
        Some(eri) => {
            let basis_functions = basis.nbasis();
            let expected = CompactEri::storage_len(basis_functions);
            if eri.len() != expected {
                return Err(EriError::InvalidValueCount {
                    basis_functions,
                    expected,
                    actual: eri.len(),
                }
                .into());
            }
            (eri, None)
        }
        None => builder.electron_repulsion()?,
    };
    if let Some(event) = eri_cache_event {
        cache_event(event);
    }
    timings.electron_repulsion_integrals = step_start.elapsed();
    timings.total = setup_start.elapsed();

    Ok(PreparedScfSetup {
        integrals: ScfIntegrals {
            overlap,
            kinetic,
            nuclear_attraction,
            electron_repulsion,
        },
        orthogonalizer,
        orthogonalization,
        timings,
    })
}
