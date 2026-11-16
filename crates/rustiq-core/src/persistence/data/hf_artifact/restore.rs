use super::{ConvergedHfArtifact, HfOrbitalChannel, HfOrbitals, RhfOrbitals, UhfOrbitals};
use crate::{
    calculation::{
        AnyHfSolution, HfSolution, OrthogonalizationInfo, PreparedCalculation, RhfSolution,
        ScfEnergyDetails, ScfResult, ScfTimings, Spin, SpinDiagnostics, UhfSolution,
    },
    config::{ResolvedHfMethod, RestrictedMethod, UnrestrictedMethod},
    hf::core::core_hamiltonian_ints,
    persistence::{AoEriArtifact, Artifact, PortableError, RestoreFromRustiQ, RustiQData},
};
use nalgebra::{DMatrix, DVector};

impl RestoreFromRustiQ for AnyHfSolution {
    fn restore_from_rustiq(data: &RustiQData) -> Result<Self, PortableError> {
        let prepared = data.clone().prepare_calculation()?;
        restore_hf(data, &prepared)
    }
}

fn restore_hf(
    data: &RustiQData,
    prepared: &PreparedCalculation,
) -> Result<AnyHfSolution, PortableError> {
    let artifact = data
        .get::<ConvergedHfArtifact>()?
        .ok_or(PortableError::MissingArtifact(ConvergedHfArtifact::NAME))?;
    if !data.hf_is_compatible(prepared) {
        return Err(PortableError::InvalidCalculation(
            "HF artifact is incompatible with the prepared calculation".into(),
        ));
    }
    if !AoEriArtifact::is_present(data) {
        return Err(PortableError::MissingArtifact(AoEriArtifact::NAME));
    }
    let integrals = data.take_compatible_eri(prepared)?;
    // The getter returns a fresh, independently owned record. Moving it into the
    // executable solution avoids copying any decoded orbital buffers.
    let record = std::sync::Arc::try_unwrap(artifact.0).map_err(|_| {
        PortableError::InvalidCalculation("decoded HF record is unexpectedly shared".into())
    })?;
    let n = usize::try_from(record.basis_dimension).map_err(|_| {
        PortableError::InvalidCalculation("HF basis dimension exceeds host range".into())
    })?;
    let rank = usize::try_from(record.effective_rank)
        .map_err(|_| PortableError::InvalidCalculation("HF rank exceeds host range".into()))?;
    let channel = |c: HfOrbitalChannel| {
        (
            DMatrix::from_vec(n, rank, c.coefficients),
            DVector::from_vec(c.energies),
            usize::try_from(c.occupied).expect("validated occupied count fits the basis dimension"),
        )
    };
    let density = |c: &DMatrix<f64>, occupied| {
        let occupied = c.columns(0, occupied);
        occupied * occupied.transpose()
    };
    let mut summary = ScfResult {
        iterations: usize::try_from(record.iterations).map_err(|_| {
            PortableError::InvalidCalculation("HF iteration count exceeds host range".into())
        })?,
        electronic_energy: record.electronic_energy,
        nuclear_repulsion_energy: record.nuclear_repulsion_energy,
        total_energy: record.total_energy,
        delta_energy: record.delta_energy,
        residual_norm: record.residual_norm,
        spin: record.spin.map(|s| SpinDiagnostics {
            s_squared: s[0],
            ideal_s_squared: s[1],
            spin_contamination: s[2],
        }),
        energy_details: ScfEnergyDetails {
            kinetic_energy: 0.0,
            nuclear_attraction_energy: 0.0,
            electron_repulsion_energy: 0.0,
        },
        orthogonalization: OrthogonalizationInfo {
            basis_dimension: n,
            effective_rank: rank,
            discarded_directions: n - rank,
            relative_threshold: record.linear_dependency_threshold,
        },
        // Execution timings are not persisted. Restoration does not represent a new SCF run.
        timings: ScfTimings::default(),
    };
    Ok(match record.orbitals {
        HfOrbitals::Rhf(RhfOrbitals { shared }) => {
            let (coefficients, energies, occupied) = channel(shared);
            summary.energy_details = energy_details(
                prepared,
                record.electronic_energy,
                &(2.0 * density(&coefficients, occupied)),
            )?;
            AnyHfSolution::Restricted(HfSolution::new(
                RestrictedMethod::Rhf,
                Some(record.identity),
                summary,
                coefficients,
                energies,
                occupied,
                integrals,
            ))
        }
        HfOrbitals::Uhf(UhfOrbitals { channels }) => {
            let (alpha_c, alpha_e, alpha_n) = channel(channels.alpha);
            let (beta_c, beta_e, beta_n) = channel(channels.beta);
            summary.energy_details = energy_details(
                prepared,
                record.electronic_energy,
                &(density(&alpha_c, alpha_n) + density(&beta_c, beta_n)),
            )?;
            AnyHfSolution::Unrestricted(HfSolution::new(
                UnrestrictedMethod::Uhf,
                Some(record.identity),
                summary,
                Spin::new(alpha_c, beta_c),
                Spin::new(alpha_e, beta_e),
                Spin::new(alpha_n, beta_n),
                integrals,
            ))
        }
    })
}

fn energy_details(
    prepared: &PreparedCalculation,
    electronic_energy: f64,
    density: &DMatrix<f64>,
) -> Result<ScfEnergyDetails, PortableError> {
    let (kinetic, attraction) =
        core_hamiltonian_ints(prepared.get_molecule().geometry(), prepared.get_basis());
    let kinetic_energy = density.dot(&kinetic);
    let nuclear_attraction_energy = density.dot(&attraction);
    let electron_repulsion_energy = electronic_energy - kinetic_energy - nuclear_attraction_energy;
    if [
        kinetic_energy,
        nuclear_attraction_energy,
        electron_repulsion_energy,
    ]
    .iter()
    .any(|value| !value.is_finite())
    {
        return Err(PortableError::InvalidCalculation(
            "restored HF energy components are not finite".into(),
        ));
    }
    Ok(ScfEnergyDetails {
        kinetic_energy,
        nuclear_attraction_energy,
        electron_repulsion_energy,
    })
}

macro_rules! restore_typed_hf {
    ($solution:ty, $method:ident, $extract:ident) => {
        impl RestoreFromRustiQ for $solution {
            fn restore_from_rustiq(data: &RustiQData) -> Result<Self, PortableError> {
                let prepared = data.clone().prepare_calculation()?;
                let actual = prepared.hf_method();
                if actual != ResolvedHfMethod::$method {
                    return Err(PortableError::HfReferenceMismatch {
                        expected: ResolvedHfMethod::$method,
                        actual,
                    });
                }
                restore_hf(data, &prepared)?.$extract().map_err(|_| {
                    PortableError::InvalidCalculation("restored HF reference changed".into())
                })
            }
        }
    };
}
restore_typed_hf!(RhfSolution, Rhf, into_restricted);
restore_typed_hf!(UhfSolution, Uhf, into_unrestricted);
