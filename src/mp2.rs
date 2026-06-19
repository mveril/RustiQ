use nalgebra::{DMatrix, DVector};
use rayon::prelude::*;
use thiserror::Error;

use crate::{
    eri::CompactEri,
    hf::{
        numerical_error::{ensure_finite_value, ensure_finite_values, NumericalError},
        scf::ScfCalculation,
        uhf::UhfCalculation,
    },
};

#[derive(Debug, Clone, Copy)]
pub struct Mp2Input<'a> {
    pub mo_coefficients: &'a DMatrix<f64>,
    pub orbital_energies: &'a DVector<f64>,
    pub occupied_orbitals: usize,
    pub frozen_orbitals: usize,
    pub two_electron_integrals: &'a CompactEri,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mp2Result {
    pub correlation_energy: f64,
    pub electronic_energy: f64,
}

#[derive(Debug, Error)]
pub enum Mp2Error {
    #[error(
        "MP2 input dimensions do not match: MO coefficients are {coeff_rows}x{coeff_cols}, orbital energies have length {energy_len}"
    )]
    DimensionMismatch {
        coeff_rows: usize,
        coeff_cols: usize,
        energy_len: usize,
    },
    #[error(
        "closed-shell MP2 requires at least one occupied and one virtual orbital, but got {occupied} occupied orbitals out of {total}"
    )]
    InvalidOrbitalPartition { occupied: usize, total: usize },
    #[error("frozen orbitals ({frozen}) must be less than occupied orbitals ({occupied})")]
    InvalidFrozenOrbitalCount { frozen: usize, occupied: usize },
    #[error(transparent)]
    Numerical(#[from] NumericalError),
}

pub fn rhf_closed_shell(
    scf: &ScfCalculation<'_>,
    frozen_orbitals: usize,
) -> Result<Mp2Result, Mp2Error> {
    let input = Mp2Input {
        mo_coefficients: &scf.mo_coefficients,
        orbital_energies: &scf.orbital_energies,
        occupied_orbitals: scf.occupied_orbitals,
        frozen_orbitals,
        two_electron_integrals: &scf.two_electron_integrals,
    };
    let correlation_energy = correlation_energy(&input)?;
    Ok(Mp2Result {
        correlation_energy,
        electronic_energy: scf.energy + correlation_energy,
    })
}

pub fn uhf_unrestricted(
    scf: &UhfCalculation<'_>,
    frozen_orbitals: usize,
) -> Result<Mp2Result, Mp2Error> {
    let alpha = Mp2SpinInput {
        mo_coefficients: &scf.mo_coefficients.alpha,
        orbital_energies: &scf.orbital_energies.alpha,
        occupied_orbitals: scf.occupied_orbitals.alpha,
        frozen_orbitals,
    };
    let beta = Mp2SpinInput {
        mo_coefficients: &scf.mo_coefficients.beta,
        orbital_energies: &scf.orbital_energies.beta,
        occupied_orbitals: scf.occupied_orbitals.beta,
        frozen_orbitals,
    };

    let correlation_energy = uhf_correlation_energy(alpha, beta, &scf.two_electron_integrals)?;

    Ok(Mp2Result {
        correlation_energy,
        electronic_energy: scf.energy + correlation_energy,
    })
}

pub(crate) fn correlation_energy(input: &Mp2Input<'_>) -> Result<f64, Mp2Error> {
    let mo_coefficients = input.mo_coefficients;
    let orbital_energies = input.orbital_energies;

    if mo_coefficients.nrows() != mo_coefficients.ncols()
        || orbital_energies.len() != mo_coefficients.ncols()
    {
        return Err(Mp2Error::DimensionMismatch {
            coeff_rows: mo_coefficients.nrows(),
            coeff_cols: mo_coefficients.ncols(),
            energy_len: orbital_energies.len(),
        });
    }

    let total_orbitals = mo_coefficients.ncols();
    let occupied_orbitals = input.occupied_orbitals;
    let frozen_orbitals = input.frozen_orbitals;
    if occupied_orbitals == 0 || occupied_orbitals >= total_orbitals {
        return Err(Mp2Error::InvalidOrbitalPartition {
            occupied: occupied_orbitals,
            total: total_orbitals,
        });
    }
    if frozen_orbitals >= occupied_orbitals {
        return Err(Mp2Error::InvalidFrozenOrbitalCount {
            frozen: frozen_orbitals,
            occupied: occupied_orbitals,
        });
    }

    ensure_finite_values(orbital_energies, "orbital energies")?;

    let ovov_integrals = build_ovov_integrals(input);
    let active_occupied_orbitals = occupied_orbitals - frozen_orbitals;
    let virtual_orbitals = total_orbitals - occupied_orbitals;

    let correlation_energy: f64 = (0..active_occupied_orbitals)
        .into_par_iter()
        .map(|i| -> Result<f64, Mp2Error> {
            let i_orbital = frozen_orbitals + i;
            let mut partial_energy = 0.0;

            for j in 0..active_occupied_orbitals {
                let j_orbital = frozen_orbitals + j;
                for a in 0..virtual_orbitals {
                    let a_orbital = occupied_orbitals + a;
                    let ia = orbital_pair_index(i, a, virtual_orbitals);
                    let ja = orbital_pair_index(j, a, virtual_orbitals);
                    for b in 0..virtual_orbitals {
                        let b_orbital = occupied_orbitals + b;
                        let ib = orbital_pair_index(i, b, virtual_orbitals);
                        let jb = orbital_pair_index(j, b, virtual_orbitals);
                        let iajb = ovov_integrals[(ia, jb)];
                        let ibja = ovov_integrals[(ib, ja)];
                        let denominator = orbital_energies[i_orbital] + orbital_energies[j_orbital]
                            - orbital_energies[a_orbital]
                            - orbital_energies[b_orbital];
                        ensure_finite_value(denominator, "MP2 energy denominator")?;

                        partial_energy += ((2.0 * iajb) - ibja) * iajb / denominator;
                    }
                }
            }

            Ok(partial_energy)
        })
        .try_reduce(|| 0.0, |left, right| Ok(left + right))?;

    ensure_finite_value(correlation_energy, "MP2 correlation energy")?;
    Ok(correlation_energy)
}

fn build_ovov_integrals(input: &Mp2Input<'_>) -> DMatrix<f64> {
    let mo_coefficients = input.mo_coefficients;
    let basis_functions = mo_coefficients.nrows();
    let total_orbitals = mo_coefficients.ncols();
    let occupied_orbitals = input.occupied_orbitals;
    let frozen_orbitals = input.frozen_orbitals;
    let active_occupied_orbitals = occupied_orbitals - frozen_orbitals;
    let virtual_orbitals = total_orbitals - occupied_orbitals;
    let pair_count = active_occupied_orbitals * virtual_orbitals;

    let ao_pair_matrix = build_ao_pair_matrix(input.two_electron_integrals, basis_functions);
    let orbital_pair_transform =
        build_orbital_pair_transform(mo_coefficients, frozen_orbitals, occupied_orbitals);

    if pair_count == 0 {
        return DMatrix::zeros(0, 0);
    }

    orbital_pair_transform.transpose() * ao_pair_matrix * orbital_pair_transform
}

fn uhf_correlation_energy(
    alpha: Mp2SpinInput<'_>,
    beta: Mp2SpinInput<'_>,
    two_electron_integrals: &CompactEri,
) -> Result<f64, Mp2Error> {
    validate_spin_input(&alpha)?;
    validate_spin_input(&beta)?;
    if alpha.frozen_orbitals > alpha.occupied_orbitals
        || beta.frozen_orbitals > beta.occupied_orbitals
    {
        return Err(Mp2Error::InvalidFrozenOrbitalCount {
            frozen: alpha.frozen_orbitals.max(beta.frozen_orbitals),
            occupied: alpha.occupied_orbitals.min(beta.occupied_orbitals),
        });
    }

    let alpha_total_orbitals = alpha.mo_coefficients.ncols();
    let beta_total_orbitals = beta.mo_coefficients.ncols();
    if alpha_total_orbitals != beta_total_orbitals {
        return Err(Mp2Error::DimensionMismatch {
            coeff_rows: alpha.mo_coefficients.nrows(),
            coeff_cols: alpha_total_orbitals,
            energy_len: beta_total_orbitals,
        });
    }

    ensure_finite_values(alpha.orbital_energies, "alpha orbital energies")?;
    ensure_finite_values(beta.orbital_energies, "beta orbital energies")?;

    let ao_pair_matrix =
        build_ao_pair_matrix(two_electron_integrals, alpha.mo_coefficients.nrows());
    let alpha_ovov = build_orbital_pair_transform(
        alpha.mo_coefficients,
        alpha.frozen_orbitals,
        alpha.occupied_orbitals,
    );
    let beta_ovov = build_orbital_pair_transform(
        beta.mo_coefficients,
        beta.frozen_orbitals,
        beta.occupied_orbitals,
    );

    let alpha_same_spin = alpha_ovov.transpose() * &ao_pair_matrix * &alpha_ovov;
    let beta_same_spin = beta_ovov.transpose() * &ao_pair_matrix * &beta_ovov;
    let alpha_beta_direct = alpha_ovov.transpose() * ao_pair_matrix * beta_ovov;

    let alpha_same_spin_energy = same_spin_correlation_energy(
        &alpha_same_spin,
        alpha.orbital_energies,
        alpha.occupied_orbitals,
        alpha.frozen_orbitals,
    )?;
    let beta_same_spin_energy = same_spin_correlation_energy(
        &beta_same_spin,
        beta.orbital_energies,
        beta.occupied_orbitals,
        beta.frozen_orbitals,
    )?;
    let opposite_spin_energy = opposite_spin_correlation_energy(
        &alpha_beta_direct,
        alpha.orbital_energies,
        alpha.occupied_orbitals,
        alpha.frozen_orbitals,
        beta.orbital_energies,
        beta.occupied_orbitals,
        beta.frozen_orbitals,
    )?;

    let correlation_energy = alpha_same_spin_energy + beta_same_spin_energy + opposite_spin_energy;
    ensure_finite_value(correlation_energy, "MP2 correlation energy")?;
    Ok(correlation_energy)
}

fn build_ao_pair_matrix(
    two_electron_integrals: &CompactEri,
    basis_functions: usize,
) -> DMatrix<f64> {
    let pair_count = basis_function_pair_count(basis_functions);
    let mut matrix = DMatrix::zeros(pair_count, pair_count);

    let rows: Vec<Vec<(usize, f64)>> = (0..pair_count)
        .into_par_iter()
        .map(|left_pair_index| {
            let (mu, nu) = basis_function_pair(left_pair_index);
            let mut row = Vec::with_capacity(left_pair_index + 1);

            for right_pair_index in 0..=left_pair_index {
                let (lambda, sigma) = basis_function_pair(right_pair_index);
                let value = two_electron_integrals[(mu, nu, lambda, sigma)];
                row.push((right_pair_index, value));
            }

            row
        })
        .collect();

    for (left_pair_index, row) in rows.into_iter().enumerate() {
        for (right_pair_index, value) in row {
            matrix[(left_pair_index, right_pair_index)] = value;
            matrix[(right_pair_index, left_pair_index)] = value;
        }
    }

    matrix
}

fn build_orbital_pair_transform(
    mo_coefficients: &DMatrix<f64>,
    occupied_start: usize,
    occupied_end: usize,
) -> DMatrix<f64> {
    let basis_functions = mo_coefficients.nrows();
    let virtual_start = occupied_end;
    let active_occupied_orbitals = occupied_end - occupied_start;
    let virtual_orbitals = mo_coefficients.ncols() - virtual_start;
    let pair_count = active_occupied_orbitals * virtual_orbitals;
    let ao_pair_count = basis_function_pair_count(basis_functions);
    let mut transform = DMatrix::zeros(ao_pair_count, pair_count);

    let columns: Vec<(usize, Vec<f64>)> = (0..pair_count)
        .into_par_iter()
        .map(|column| {
            let i = column / virtual_orbitals;
            let a = column % virtual_orbitals;
            let i_orbital = occupied_start + i;
            let a_orbital = virtual_start + a;
            let mut values = Vec::with_capacity(ao_pair_count);

            for ao_pair_index in 0..ao_pair_count {
                let (mu, nu) = basis_function_pair(ao_pair_index);
                values.push(pair_transform_coefficient(
                    mo_coefficients,
                    mu,
                    nu,
                    i_orbital,
                    a_orbital,
                ));
            }

            (column, values)
        })
        .collect();

    for (column, values) in columns {
        for (ao_pair_index, value) in values.into_iter().enumerate() {
            transform[(ao_pair_index, column)] = value;
        }
    }

    transform
}

fn validate_spin_input(input: &Mp2SpinInput<'_>) -> Result<(), Mp2Error> {
    if input.mo_coefficients.nrows() != input.mo_coefficients.ncols()
        || input.orbital_energies.len() != input.mo_coefficients.ncols()
    {
        return Err(Mp2Error::DimensionMismatch {
            coeff_rows: input.mo_coefficients.nrows(),
            coeff_cols: input.mo_coefficients.ncols(),
            energy_len: input.orbital_energies.len(),
        });
    }

    let total_orbitals = input.mo_coefficients.ncols();
    if total_orbitals == 0 || input.occupied_orbitals > total_orbitals {
        return Err(Mp2Error::InvalidOrbitalPartition {
            occupied: input.occupied_orbitals,
            total: total_orbitals,
        });
    }

    Ok(())
}

fn same_spin_correlation_energy(
    ovov_integrals: &DMatrix<f64>,
    orbital_energies: &DVector<f64>,
    occupied_orbitals: usize,
    frozen_orbitals: usize,
) -> Result<f64, Mp2Error> {
    let active_occupied_orbitals = occupied_orbitals - frozen_orbitals;
    let virtual_orbitals = orbital_energies.len() - occupied_orbitals;

    let correlation_energy: f64 = (0..active_occupied_orbitals)
        .into_par_iter()
        .map(|i| -> Result<f64, Mp2Error> {
            let i_orbital = frozen_orbitals + i;
            let mut partial_energy = 0.0;

            for j in 0..active_occupied_orbitals {
                let j_orbital = frozen_orbitals + j;
                for a in 0..virtual_orbitals {
                    let a_orbital = occupied_orbitals + a;
                    let ia = orbital_pair_index(i, a, virtual_orbitals);
                    let ja = orbital_pair_index(j, a, virtual_orbitals);
                    for b in 0..virtual_orbitals {
                        let b_orbital = occupied_orbitals + b;
                        let ib = orbital_pair_index(i, b, virtual_orbitals);
                        let jb = orbital_pair_index(j, b, virtual_orbitals);
                        let direct = ovov_integrals[(ia, jb)];
                        let exchange = ovov_integrals[(ib, ja)];
                        let denominator = orbital_energies[i_orbital] + orbital_energies[j_orbital]
                            - orbital_energies[a_orbital]
                            - orbital_energies[b_orbital];
                        ensure_finite_value(denominator, "MP2 energy denominator")?;

                        partial_energy += 0.5 * direct * (direct - exchange) / denominator;
                    }
                }
            }

            Ok(partial_energy)
        })
        .try_reduce(|| 0.0, |left, right| Ok(left + right))?;

    ensure_finite_value(correlation_energy, "MP2 correlation energy")?;
    Ok(correlation_energy)
}

fn opposite_spin_correlation_energy(
    ovov_integrals: &DMatrix<f64>,
    alpha_orbital_energies: &DVector<f64>,
    alpha_occupied_orbitals: usize,
    alpha_frozen_orbitals: usize,
    beta_orbital_energies: &DVector<f64>,
    beta_occupied_orbitals: usize,
    beta_frozen_orbitals: usize,
) -> Result<f64, Mp2Error> {
    let alpha_active_occupied_orbitals = alpha_occupied_orbitals - alpha_frozen_orbitals;
    let beta_active_occupied_orbitals = beta_occupied_orbitals - beta_frozen_orbitals;
    let alpha_virtual_orbitals = alpha_orbital_energies.len() - alpha_occupied_orbitals;
    let beta_virtual_orbitals = beta_orbital_energies.len() - beta_occupied_orbitals;

    let correlation_energy: f64 = (0..alpha_active_occupied_orbitals)
        .into_par_iter()
        .map(|i| -> Result<f64, Mp2Error> {
            let i_orbital = alpha_frozen_orbitals + i;
            let mut partial_energy = 0.0;

            for j in 0..beta_active_occupied_orbitals {
                let j_orbital = beta_frozen_orbitals + j;
                for a in 0..alpha_virtual_orbitals {
                    let a_orbital = alpha_occupied_orbitals + a;
                    let ia = orbital_pair_index(i, a, alpha_virtual_orbitals);
                    for b in 0..beta_virtual_orbitals {
                        let b_orbital = beta_occupied_orbitals + b;
                        let jb = orbital_pair_index(j, b, beta_virtual_orbitals);
                        let direct = ovov_integrals[(ia, jb)];
                        let denominator = alpha_orbital_energies[i_orbital]
                            + beta_orbital_energies[j_orbital]
                            - alpha_orbital_energies[a_orbital]
                            - beta_orbital_energies[b_orbital];
                        ensure_finite_value(denominator, "MP2 energy denominator")?;

                        partial_energy += direct * direct / denominator;
                    }
                }
            }

            Ok(partial_energy)
        })
        .try_reduce(|| 0.0, |left, right| Ok(left + right))?;

    ensure_finite_value(correlation_energy, "MP2 correlation energy")?;
    Ok(correlation_energy)
}

#[derive(Debug, Clone, Copy)]
struct Mp2SpinInput<'a> {
    mo_coefficients: &'a DMatrix<f64>,
    orbital_energies: &'a DVector<f64>,
    occupied_orbitals: usize,
    frozen_orbitals: usize,
}

fn pair_transform_coefficient(
    mo_coefficients: &DMatrix<f64>,
    mu: usize,
    nu: usize,
    p: usize,
    q: usize,
) -> f64 {
    let direct = mo_coefficients[(mu, p)] * mo_coefficients[(nu, q)];
    if mu == nu {
        direct
    } else {
        direct + mo_coefficients[(nu, p)] * mo_coefficients[(mu, q)]
    }
}

fn orbital_pair_index(left: usize, right: usize, right_count: usize) -> usize {
    left * right_count + right
}

fn basis_function_pair(pair_index: usize) -> (usize, usize) {
    let first = (((8 * pair_index + 1) as f64).sqrt() as usize - 1) / 2;
    let second = pair_index - first * (first + 1) / 2;
    (first, second)
}

fn basis_function_pair_count(basis_functions: usize) -> usize {
    basis_functions * (basis_functions + 1) / 2
}

#[allow(dead_code)]
fn mo_two_electron_integral(
    two_electron_integrals: &CompactEri,
    mo_coefficients: &DMatrix<f64>,
    p: usize,
    q: usize,
    r: usize,
    s: usize,
) -> f64 {
    let basis_functions = mo_coefficients.nrows();
    let mut value = 0.0;

    for mu in 0..basis_functions {
        let c_mu_p = mo_coefficients[(mu, p)];
        for nu in 0..basis_functions {
            let c_nu_q = mo_coefficients[(nu, q)];
            let left = c_mu_p * c_nu_q;
            for lambda in 0..basis_functions {
                let c_lambda_r = mo_coefficients[(lambda, r)];
                for sigma in 0..basis_functions {
                    value += left
                        * c_lambda_r
                        * mo_coefficients[(sigma, s)]
                        * two_electron_integrals[(mu, nu, lambda, sigma)];
                }
            }
        }
    }

    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{molecules::molecule::Molecule, test_utils};
    use approx::assert_abs_diff_eq;

    #[test]
    fn test_closed_shell_mp2_distinguishes_iajb_from_ijab() {
        let mo_coefficients = DMatrix::identity(2, 2);
        let orbital_energies = DVector::from_vec(vec![-1.0, 1.0]);
        let mut eri = CompactEri::Zeroed(2);
        eri[(0, 1, 0, 1)] = 1.0;

        let input = Mp2Input {
            mo_coefficients: &mo_coefficients,
            orbital_energies: &orbital_energies,
            occupied_orbitals: 1,
            frozen_orbitals: 0,
            two_electron_integrals: &eri,
        };

        let correlation_energy = correlation_energy(&input).unwrap();
        let denominator = 2.0 * orbital_energies[0] - 2.0 * orbital_energies[1];
        let expected = 1.0 / denominator;

        assert_abs_diff_eq!(correlation_energy, expected, epsilon = 1e-12);
        assert!(correlation_energy < 0.0);
    }

    #[test]
    fn test_closed_shell_mp2_is_zero_without_virtual_coupling() {
        let mo_coefficients = DMatrix::identity(2, 2);
        let orbital_energies = DVector::from_vec(vec![-1.0, 0.5]);
        let eri = CompactEri::Zeroed(2);

        let input = Mp2Input {
            mo_coefficients: &mo_coefficients,
            orbital_energies: &orbital_energies,
            occupied_orbitals: 1,
            frozen_orbitals: 0,
            two_electron_integrals: &eri,
        };

        let correlation_energy = correlation_energy(&input).unwrap();
        assert_abs_diff_eq!(correlation_energy, 0.0, epsilon = 1e-12);
    }

    #[test]
    fn test_uhf_unrestricted_mp2_allows_empty_beta_sector() {
        let mo_coefficients = DMatrix::identity(2, 2);
        let alpha_orbital_energies = DVector::from_vec(vec![-1.0, 0.5]);
        let beta_orbital_energies = DVector::from_vec(vec![0.25, 0.75]);
        let eri = CompactEri::Zeroed(2);

        let alpha = Mp2SpinInput {
            mo_coefficients: &mo_coefficients,
            orbital_energies: &alpha_orbital_energies,
            occupied_orbitals: 1,
            frozen_orbitals: 0,
        };
        let beta = Mp2SpinInput {
            mo_coefficients: &mo_coefficients,
            orbital_energies: &beta_orbital_energies,
            occupied_orbitals: 0,
            frozen_orbitals: 0,
        };

        let correlation_energy = uhf_correlation_energy(alpha, beta, &eri).unwrap();
        assert_abs_diff_eq!(correlation_energy, 0.0, epsilon = 1e-12);
    }

    #[test]
    fn test_uhf_unrestricted_mp2_allows_spin_sector_without_virtuals() {
        let mo_coefficients = DMatrix::identity(2, 2);
        let alpha_orbital_energies = DVector::from_vec(vec![-1.0, -0.5]);
        let beta_orbital_energies = DVector::from_vec(vec![-0.75, 0.25]);
        let eri = CompactEri::Zeroed(2);

        let alpha = Mp2SpinInput {
            mo_coefficients: &mo_coefficients,
            orbital_energies: &alpha_orbital_energies,
            occupied_orbitals: 2,
            frozen_orbitals: 0,
        };
        let beta = Mp2SpinInput {
            mo_coefficients: &mo_coefficients,
            orbital_energies: &beta_orbital_energies,
            occupied_orbitals: 1,
            frozen_orbitals: 0,
        };

        let correlation_energy = uhf_correlation_energy(alpha, beta, &eri).unwrap();
        assert_abs_diff_eq!(correlation_energy, 0.0, epsilon = 1e-12);
    }

    #[test]
    fn test_uhf_unrestricted_mp2_allows_fully_frozen_spin_sector() {
        let mo_coefficients = DMatrix::identity(3, 3);
        let alpha_orbital_energies = DVector::from_vec(vec![-1.0, -0.5, 0.5]);
        let beta_orbital_energies = DVector::from_vec(vec![-0.75, 0.25, 0.75]);
        let eri = CompactEri::Zeroed(3);

        let alpha = Mp2SpinInput {
            mo_coefficients: &mo_coefficients,
            orbital_energies: &alpha_orbital_energies,
            occupied_orbitals: 2,
            frozen_orbitals: 1,
        };
        let beta = Mp2SpinInput {
            mo_coefficients: &mo_coefficients,
            orbital_energies: &beta_orbital_energies,
            occupied_orbitals: 1,
            frozen_orbitals: 1,
        };

        let correlation_energy = uhf_correlation_energy(alpha, beta, &eri).unwrap();
        assert_abs_diff_eq!(correlation_energy, 0.0, epsilon = 1e-12);
    }

    #[test]
    fn test_rhf_closed_shell_mp2_h2_sto3g_matches_pyscf_reference() {
        let geometry = test_utils::load_sample_geometry_in_bohr("samples/h2/molecule.xyz");
        let molecule = Molecule::from(geometry);
        let basis = test_utils::load_sto3g_basis(&molecule.geometry);
        let mut scf = test_utils::new_one_electron_scf(&molecule, &basis, 100, 1e-12);

        let scf_result = scf.run().unwrap();
        assert_abs_diff_eq!(
            scf_result.electronic_energy,
            -1.831_863_646_477_507,
            epsilon = 1e-10
        );

        let mp2_result = rhf_closed_shell(&scf, 0).unwrap();
        assert_abs_diff_eq!(
            mp2_result.correlation_energy,
            -0.013_138_073_589_533,
            epsilon = 1e-11
        );
    }

    #[test]
    fn test_uhf_unrestricted_mp2_oh_sto3g_matches_pyscf_reference() {
        use crate::hf::density_guess::core_hamiltonian::CoreHamiltonian;

        let geometry = test_utils::load_sample_geometry_in_bohr("samples/oh/oh.xyz");
        let basis = test_utils::load_sto3g_basis(&geometry);
        // SAFETY: Neutral OH has nine electrons.
        let molecule = unsafe {
            Molecule::new_unchecked(
                geometry,
                crate::molecules::units::Units::Bohr,
                0,
                std::num::NonZeroU8::new(2).unwrap(),
            )
        };
        let mut uhf = crate::hf::uhf::UhfCalculation::new(
            &molecule,
            &basis,
            500,
            1e-8,
            CoreHamiltonian::default(),
        )
        .unwrap();
        uhf.enable_diis(6).unwrap();

        let result = uhf.run().unwrap();
        assert!(result.converged);
        assert_abs_diff_eq!(
            result.electronic_energy,
            -78.727_017_326_066_203,
            epsilon = 5e-7
        );

        let mp2_result = uhf_unrestricted(&uhf, 0).unwrap();
        assert_abs_diff_eq!(
            mp2_result.correlation_energy,
            -0.015_810_842_704_457_297,
            epsilon = 5e-9
        );
        assert_abs_diff_eq!(
            mp2_result.electronic_energy,
            -78.742_828_168_770_66,
            epsilon = 5e-8
        );
    }
}
