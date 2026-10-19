//! Four-index contraction with a bounded occupied-orbital workspace.
use nalgebra::{DMatrix, DMatrixView, DMatrixViewMut};
use rayon::prelude::*;

use super::{ensure_finite_value, validate_denominator, CompactEri, Mp2Error, Mp2SpinInput};

/// Estimated matrix payloads, not a bound on process RSS or allocator overhead.
#[derive(Debug, Clone, Copy)]
pub struct Mp2MemoryPlan {
    pub sector: Mp2Sector,
    pub basis_functions: usize,
    pub left_occupied: usize,
    pub right_occupied: usize,
    pub left_virtual: usize,
    pub right_virtual: usize,
    pub block_size: usize,
    pub budget_bytes: u64,
    pub workspace_bytes: u64,
    pub dense_workspace_bytes: u64,
    /// ERIs, coefficients and orbital energies borrowed by this contraction.
    pub resident_input_bytes: u64,
}

/// Spin sector whose workspace is described by an [`Mp2MemoryPlan`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mp2Sector {
    Rhf,
    UhfAlphaAlpha,
    UhfBetaBeta,
    UhfAlphaBeta,
}

impl std::fmt::Display for Mp2Sector {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Rhf => "RHF",
            Self::UhfAlphaAlpha => "UHF alpha-alpha",
            Self::UhfBetaBeta => "UHF beta-beta",
            Self::UhfAlphaBeta => "UHF alpha-beta",
        })
    }
}

#[derive(Clone, Copy)]
struct Dimensions {
    n: usize,
    left: usize,
    right: usize,
    va: usize,
    vb: usize,
}

fn product(values: &[usize]) -> Result<usize, Mp2Error> {
    values.iter().try_fold(1usize, |a, &b| {
        a.checked_mul(b).ok_or(Mp2Error::SizeOverflow)
    })
}

fn sum(values: &[usize]) -> Result<usize, Mp2Error> {
    values.iter().try_fold(0usize, |a, &b| {
        a.checked_add(b).ok_or(Mp2Error::SizeOverflow)
    })
}

fn bytes(elements: usize) -> Result<u64, Mp2Error> {
    let bytes = product(&[elements, size_of::<f64>()])?;
    if bytes > isize::MAX as usize {
        return Err(Mp2Error::SizeOverflow);
    }
    Ok(bytes as u64)
}

impl Dimensions {
    fn workspace(self, b: usize) -> Result<u64, Mp2Error> {
        let Self {
            n,
            right: o,
            va,
            vb,
            ..
        } = self;
        let t1 = product(&[b, n, n, n])?;
        let t2 = product(&[b, o, n, n])?;
        let t3 = product(&[b, o, va, n])?;
        let t4 = product(&[b, o, va, vb])?;
        let peak = [
            sum(&[t1, product(&[n, n])?])?,
            sum(&[t1, t2])?,
            sum(&[t2, t3, product(&[n, b])?])?,
            sum(&[t3, t4])?,
        ]
        .into_iter()
        .max()
        .expect("the workspace estimate contains four allocation stages");
        let coefficients = product(&[n, sum(&[b, o, va, vb])?])?;
        bytes(sum(&[peak, coefficients])?)
    }

    fn block(self, budget: u64) -> Result<(usize, u64), Mp2Error> {
        if budget == 0 {
            return Err(Mp2Error::InvalidMemoryLimit);
        }
        let required = self.workspace(1)?;
        if required > budget {
            return Err(Mp2Error::InsufficientMemory { required, budget });
        }
        let (mut lo, mut hi) = (1, self.left);
        while lo < hi {
            let mid = lo + (hi - lo).div_ceil(2);
            match self.workspace(mid) {
                Ok(size) if size <= budget => lo = mid,
                _ => hi = mid - 1,
            }
        }
        Ok((lo, self.workspace(lo)?))
    }

    // Upper bound for the previous dense contraction, including its transpose.
    fn dense(self) -> Result<u64, Mp2Error> {
        let p = product(&[self.n, sum(&[self.n, 1])?])? / 2;
        let left = product(&[self.left, self.va])?;
        let right = product(&[self.right, self.vb])?;
        bytes(sum(&[
            product(&[p, p])?,
            product(&[p, left])?,
            product(&[p, right])?,
            product(&[p, left])?,
            product(&[p, left])?,
            product(&[left, right])?,
        ])?)
    }
}

pub(super) fn rhf_workspace_for_block(
    n: usize,
    occupied: usize,
    block_size: usize,
) -> Result<u64, Mp2Error> {
    Dimensions {
        n,
        left: occupied,
        right: occupied,
        va: n.checked_sub(occupied).ok_or(Mp2Error::SizeOverflow)?,
        vb: n.checked_sub(occupied).ok_or(Mp2Error::SizeOverflow)?,
    }
    .workspace(block_size)
}

/// Conservative peak bound for the old UHF implementation, which retained
/// both same-spin tensors while constructing the opposite-spin tensor.
pub(super) fn uhf_dense_bytes(
    alpha: Mp2SpinInput<'_>,
    beta: Mp2SpinInput<'_>,
) -> Result<u64, Mp2Error> {
    let n = alpha.mo_coefficients.nrows();
    let p = product(&[n, sum(&[n, 1])?])? / 2;
    let a = product(&[
        alpha.occupied_orbitals - alpha.frozen_orbitals,
        alpha.mo_coefficients.ncols() - alpha.occupied_orbitals,
    ])?;
    let b = product(&[
        beta.occupied_orbitals - beta.frozen_orbitals,
        beta.mo_coefficients.ncols() - beta.occupied_orbitals,
    ])?;
    bytes(sum(&[
        product(&[p, p])?,
        product(&[p, sum(&[a, b])?])?,
        product(&[a, a])?,
        product(&[b, b])?,
        product(&[a, b])?,
        product(&[2, p, a.max(b)])?,
    ])?)
}

pub(super) fn energy(
    left: Mp2SpinInput<'_>,
    right: Mp2SpinInput<'_>,
    eri: &CompactEri,
    budget: u64,
    sector: Mp2Sector,
    report: &mut impl FnMut(Mp2MemoryPlan),
) -> Result<f64, Mp2Error> {
    if budget == 0 {
        return Err(Mp2Error::InvalidMemoryLimit);
    }
    let d = Dimensions {
        n: left.mo_coefficients.nrows(),
        left: left.occupied_orbitals - left.frozen_orbitals,
        right: right.occupied_orbitals - right.frozen_orbitals,
        va: left.mo_coefficients.ncols() - left.occupied_orbitals,
        vb: right.mo_coefficients.ncols() - right.occupied_orbitals,
    };
    if d.left == 0
        || d.right == 0
        || d.va == 0
        || d.vb == 0
        || (matches!(sector, Mp2Sector::UhfAlphaAlpha | Mp2Sector::UhfBetaBeta)
            && (d.left < 2 || d.va < 2))
    {
        return Ok(0.0);
    }
    let (block_size, workspace_bytes) = d.block(budget)?;
    let same_coefficients = std::ptr::eq(left.mo_coefficients, right.mo_coefficients);
    let resident = sum(&[
        eri.len(),
        left.mo_coefficients.len(),
        left.orbital_energies.len(),
        if same_coefficients {
            0
        } else {
            right.mo_coefficients.len()
        },
        if std::ptr::eq(left.orbital_energies, right.orbital_energies) {
            0
        } else {
            right.orbital_energies.len()
        },
    ])?;
    report(Mp2MemoryPlan {
        sector,
        basis_functions: d.n,
        left_occupied: d.left,
        right_occupied: d.right,
        left_virtual: d.va,
        right_virtual: d.vb,
        block_size,
        budget_bytes: budget,
        workspace_bytes,
        dense_workspace_bytes: d.dense()?,
        resident_input_bytes: bytes(resident)?,
    });
    let staged = StagedCoefficients::new(left, right);
    let mut total = 0.0;
    for start in (0..d.left).step_by(block_size) {
        let b = block_size.min(d.left - start);
        let (integrals, actual_payload) = transform(left, &staged, eri, d, start, b);
        debug_assert!(actual_payload <= workspace_bytes);
        for i in 0..b {
            let ii = left.frozen_orbitals + start + i;
            for j in 0..d.right {
                let jj = right.frozen_orbitals + j;
                for a in 0..d.va {
                    for v in 0..d.vb {
                        if matches!(sector, Mp2Sector::UhfAlphaAlpha | Mp2Sector::UhfBetaBeta)
                            && (start + i == j || a == v)
                        {
                            continue;
                        }
                        let direct = integrals[(a + d.va * (i + b * j), v)];
                        let denominator = left.orbital_energies[ii] + right.orbital_energies[jj]
                            - left.orbital_energies[left.occupied_orbitals + a]
                            - right.orbital_energies[right.occupied_orbitals + v];
                        validate_denominator(denominator)?;
                        let numerator = match sector {
                            Mp2Sector::UhfAlphaBeta => direct * direct,
                            Mp2Sector::Rhf | Mp2Sector::UhfAlphaAlpha | Mp2Sector::UhfBetaBeta => {
                                let exchange = integrals[(v + d.va * (i + b * j), a)];
                                if matches!(sector, Mp2Sector::Rhf) {
                                    direct * (2.0 * direct - exchange)
                                } else {
                                    0.5 * direct * (direct - exchange)
                                }
                            }
                        };
                        total += numerator / denominator;
                    }
                }
            }
        }
    }
    ensure_finite_value(total, "MP2 correlation energy")?;
    Ok(total)
}

// These columns are invariant across occupied blocks. Only the left virtual
// transpose needs staging; contiguous right columns can borrow the input.
struct StagedCoefficients<'a> {
    cj: DMatrixView<'a, f64>,
    ca: DMatrix<f64>,
    cb: DMatrixView<'a, f64>,
}

impl<'a> StagedCoefficients<'a> {
    fn new(left: Mp2SpinInput<'a>, right: Mp2SpinInput<'a>) -> Self {
        Self {
            cj: right.mo_coefficients.columns(
                right.frozen_orbitals,
                right.occupied_orbitals - right.frozen_orbitals,
            ),
            ca: left
                .mo_coefficients
                .columns(
                    left.occupied_orbitals,
                    left.mo_coefficients.ncols() - left.occupied_orbitals,
                )
                .transpose(),
            cb: right.mo_coefficients.columns(
                right.occupied_orbitals,
                right.mo_coefficients.ncols() - right.occupied_orbitals,
            ),
        }
    }
}

fn transform(
    left: Mp2SpinInput<'_>,
    staged: &StagedCoefficients<'_>,
    eri: &CompactEri,
    d: Dimensions,
    start: usize,
    b: usize,
) -> (DMatrix<f64>, u64) {
    let Dimensions {
        n,
        right: o,
        va,
        vb,
        ..
    } = d;
    let ci = left
        .mo_coefficients
        .columns(left.frozen_orbitals + start, b)
        .transpose();
    let StagedCoefficients { cj, ca, cb } = staged;
    let coefficients = ci.len() + ca.len();
    let mut peak;

    // Storage order: i, nu, lambda, sigma. Only one N x N AO panel is expanded.
    let mut t1 = DMatrix::zeros(b, n * n * n);
    {
        let mut panel = DMatrix::zeros(n, n);
        peak = coefficients + t1.len() + panel.len();
        for sigma in 0..n {
            for lambda in 0..=sigma {
                // Compact ERIs are symmetric in the first AO pair. Expand
                // both triangles from one lookup without changing GEMM order.
                for nu in 0..n {
                    for mu in 0..=nu {
                        let value = eri[(mu, nu, lambda, sigma)];
                        panel[(mu, nu)] = value;
                        panel[(nu, mu)] = value;
                    }
                }
                t1.columns_mut(n * (lambda + n * sigma), n)
                    .gemm(1.0, &ci, &panel, 0.0);
                // Symmetry of the second AO pair survives the mu contraction:
                // reuse the transformed panel for the swapped lambda/sigma pair.
                if lambda != sigma {
                    for nu in 0..n {
                        let source = nu + n * (lambda + n * sigma);
                        let target = nu + n * (sigma + n * lambda);
                        for i in 0..b {
                            t1[(i, target)] = t1[(i, source)];
                        }
                    }
                }
            }
        }
    }
    // i, nu, j, sigma: transform lambda while keeping contiguous panels.
    let mut t2 = DMatrix::zeros(b * n, o * n);
    peak = peak.max(coefficients + t1.len() + t2.len());
    t2.as_mut_slice()
        .par_chunks_mut(b * n * o)
        .enumerate()
        .for_each(|(sigma, output)| {
            let panel = DMatrixView::from_slice(&t1.as_slice()[sigma * b * n * n..], b * n, n);
            DMatrixViewMut::from_slice(output, b * n, o).gemm(1.0, &panel, cj, 0.0);
        });
    drop(t1);
    // a, i, j, sigma: a small transposed view avoids a full tensor permutation.
    let mut t3 = DMatrix::zeros(va * b * o, n);
    peak = peak.max(coefficients + t2.len() + t3.len() + n * b);
    for sigma in 0..n {
        for j in 0..o {
            let panel = DMatrixView::from_slice(&t2.as_slice()[(j + o * sigma) * b * n..], b, n);
            let mut output = DMatrixViewMut::from_slice(
                &mut t3.as_mut_slice()[(j + o * sigma) * va * b..],
                va,
                b,
            );
            output.gemm(1.0, ca, &panel.transpose(), 0.0);
        }
    }
    drop(t2);
    let mut t4 = DMatrix::zeros(va * b * o, vb);
    peak = peak.max(coefficients + t3.len() + t4.len());
    t4.gemm(1.0, &t3, cb, 0.0);
    (t4, (peak * size_of::<f64>()) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eri::index::EriIndex;
    use approx::assert_relative_eq;
    use nalgebra::DVector;

    #[allow(
        clippy::cast_precision_loss,
        reason = "Fixture dimensions and quadrature orders are small enough to be represented exactly in f64"
    )]
    fn coefficients(n: usize, m: usize, shift: f64) -> DMatrix<f64> {
        DMatrix::from_fn(n, m, |i, j| ((i * m + j) as f64 + shift).sin() / n as f64)
    }

    #[allow(
        clippy::cast_precision_loss,
        reason = "Fixture dimensions and quadrature orders are small enough to be represented exactly in f64"
    )]
    fn integrals(n: usize) -> CompactEri {
        let mut eri = CompactEri::Zeroed(n);
        for k in 0..eri.len() {
            eri[EriIndex(k)] = (k as f64 * 0.7).cos() / 3.0;
        }
        eri
    }

    #[allow(
        clippy::many_single_char_names,
        reason = "Indices and exponents follow the standard notation for this integral or contraction"
    )]
    fn direct(
        left: &DMatrix<f64>,
        right: &DMatrix<f64>,
        eri: &CompactEri,
        indices: [usize; 4],
    ) -> f64 {
        let [i, a, j, b] = indices;
        let n = left.nrows();
        let mut result = 0.0;
        for mu in 0..n {
            for nu in 0..n {
                for lambda in 0..n {
                    for sigma in 0..n {
                        result += left[(mu, i)]
                            * left[(nu, a)]
                            * right[(lambda, j)]
                            * right[(sigma, b)]
                            * eri[(mu, nu, lambda, sigma)];
                    }
                }
            }
        }
        result
    }

    #[test]
    fn staged_coefficients_borrow_untransposed_columns_and_reuse_virtual_transpose() {
        let ca = coefficients(7, 6, 0.3);
        let cb = coefficients(7, 6, 1.2);
        let energies = DVector::from_element(6, 1.0);
        let left = Mp2SpinInput {
            mo_coefficients: &ca,
            orbital_energies: &energies,
            occupied_orbitals: 4,
            frozen_orbitals: 1,
        };
        let right = Mp2SpinInput {
            mo_coefficients: &cb,
            occupied_orbitals: 3,
            ..left
        };
        let staged = StagedCoefficients::new(left, right);
        assert_eq!(staged.cj.as_ptr(), cb.column(1).as_ptr());
        assert_eq!(staged.cb.as_ptr(), cb.column(3).as_ptr());
        assert_eq!(staged.ca, ca.columns(4, 2).transpose());
        let d = Dimensions {
            n: 7,
            left: 3,
            right: 2,
            va: 2,
            vb: 3,
        };
        let eri = integrals(7);
        let transpose_ptr = staged.ca.as_ptr();
        for start in 0..d.left {
            let (actual, payload) = transform(left, &staged, &eri, d, start, 1);
            assert!(payload <= d.workspace(1).unwrap());
            assert_eq!(staged.ca.as_ptr(), transpose_ptr);
            assert_relative_eq!(
                actual[(0, 0)],
                direct(&ca, &cb, &eri, [1 + start, 4, 1, 3]),
                epsilon = 1e-12
            );
        }
    }

    #[test]
    fn blocked_integrals_match_independent_four_index_contraction() {
        for threads in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            pool.install(|| {
                let n = 7;
                let ca = coefficients(n, 6, 0.3);
                let cb = coefficients(n, 6, 1.2);
                let e = DVector::from_vec(vec![-3., -2., -1., -0.5, 0.4, 1.]);
                let eri = integrals(n);
                for frozen in [0, 1] {
                    let left = Mp2SpinInput {
                        mo_coefficients: &ca,
                        orbital_energies: &e,
                        occupied_orbitals: 4,
                        frozen_orbitals: frozen,
                    };
                    let right = Mp2SpinInput {
                        mo_coefficients: &cb,
                        orbital_energies: &e,
                        occupied_orbitals: 3,
                        frozen_orbitals: frozen,
                    };
                    let d = Dimensions {
                        n,
                        left: 4 - frozen,
                        right: 3 - frozen,
                        va: 2,
                        vb: 3,
                    };
                    let staged = StagedCoefficients::new(left, right);
                    for block in [1, 2, 3, d.left] {
                        for start in (0..d.left).step_by(block) {
                            let b = block.min(d.left - start);
                            let (actual, payload) = transform(left, &staged, &eri, d, start, b);
                            assert!(payload <= d.workspace(b).unwrap());
                            for i in 0..b {
                                for j in 0..d.right {
                                    for a in 0..d.va {
                                        for v in 0..d.vb {
                                            let expected = direct(
                                                &ca,
                                                &cb,
                                                &eri,
                                                [frozen + start + i, 4 + a, frozen + j, 3 + v],
                                            );
                                            assert_relative_eq!(
                                                actual[(a + d.va * (i + b * j), v)],
                                                expected,
                                                epsilon = 1e-12,
                                                max_relative = 1e-12
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            });
        }
    }

    fn rhf_dense_energy(
        dense: &DMatrix<f64>,
        energies: &DVector<f64>,
        occupied: usize,
        frozen: usize,
        d: Dimensions,
    ) -> f64 {
        let mut value = 0.0;
        for i in 0..d.left {
            for j in 0..d.right {
                for a in 0..d.va {
                    for b in 0..d.vb {
                        let direct = dense[(i * d.va + a, j * d.vb + b)];
                        let exchange = dense[(i * d.va + b, j * d.vb + a)];
                        value += direct * (2.0 * direct - exchange)
                            / (energies[frozen + i] + energies[frozen + j]
                                - energies[occupied + a]
                                - energies[occupied + b]);
                    }
                }
            }
        }
        value
    }

    #[test]
    fn blocked_energies_match_dense_reference_for_every_sector_and_block_size() {
        let n = 7;
        let ca = coefficients(n, 6, 0.3);
        let cb = coefficients(n, 6, 1.2);
        let ea = DVector::from_vec(vec![-3., -2., -1., -0.5, 0.4, 1.]);
        let eb = DVector::from_vec(vec![-2.8, -1.8, -0.9, 0.2, 0.6, 1.2]);
        let eri = integrals(n);
        for frozen in [0, 1] {
            let alpha = Mp2SpinInput {
                mo_coefficients: &ca,
                orbital_energies: &ea,
                occupied_orbitals: 4,
                frozen_orbitals: frozen,
            };
            let beta = Mp2SpinInput {
                mo_coefficients: &cb,
                orbital_energies: &eb,
                occupied_orbitals: 3,
                frozen_orbitals: frozen,
            };
            for sector in [
                Mp2Sector::Rhf,
                Mp2Sector::UhfAlphaAlpha,
                Mp2Sector::UhfBetaBeta,
                Mp2Sector::UhfAlphaBeta,
            ] {
                let left = if matches!(sector, Mp2Sector::UhfBetaBeta) {
                    beta
                } else {
                    alpha
                };
                let right = if matches!(sector, Mp2Sector::UhfAlphaBeta) {
                    beta
                } else {
                    left
                };
                let d = Dimensions {
                    n,
                    left: left.occupied_orbitals - frozen,
                    right: right.occupied_orbitals - frozen,
                    va: 6 - left.occupied_orbitals,
                    vb: 6 - right.occupied_orbitals,
                };
                let tl = super::super::build_orbital_pair_transform(
                    left.mo_coefficients,
                    frozen,
                    left.occupied_orbitals,
                );
                let tr = super::super::build_orbital_pair_transform(
                    right.mo_coefficients,
                    frozen,
                    right.occupied_orbitals,
                );
                let dense = tl.transpose() * super::super::build_ao_pair_matrix(&eri, n) * tr;
                let expected = match sector {
                    Mp2Sector::UhfAlphaAlpha | Mp2Sector::UhfBetaBeta => {
                        super::super::same_spin_correlation_energy(
                            &dense,
                            left.orbital_energies,
                            left.occupied_orbitals,
                            frozen,
                        )
                        .unwrap()
                    }
                    Mp2Sector::UhfAlphaBeta => super::super::opposite_spin_correlation_energy(
                        &dense, &ea, 4, frozen, &eb, 3, frozen,
                    )
                    .unwrap(),
                    Mp2Sector::Rhf => rhf_dense_energy(&dense, &ea, 4, frozen, d),
                };
                for threads in [1, 4] {
                    let pool = rayon::ThreadPoolBuilder::new()
                        .num_threads(threads)
                        .build()
                        .unwrap();
                    for b in [1, 2, 3, d.left].into_iter().filter(|&b| b <= d.left) {
                        let budget = d.workspace(b).unwrap();
                        let actual = pool
                            .install(|| {
                                energy(left, right, &eri, budget, sector, &mut |p| {
                                    assert_eq!(p.block_size, b);
                                    assert!(p.workspace_bytes <= budget);
                                })
                            })
                            .unwrap();
                        assert_relative_eq!(
                            actual,
                            expected,
                            epsilon = 1e-12,
                            max_relative = 1e-12
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn planner_checks_boundaries_and_overflow_without_allocating() {
        let d = Dimensions {
            n: 12,
            left: 5,
            right: 4,
            va: 7,
            vb: 8,
        };
        let minimum = d.workspace(1).unwrap();
        assert!(matches!(d.block(0), Err(Mp2Error::InvalidMemoryLimit)));
        assert!(matches!(
            d.block(minimum - 1),
            Err(Mp2Error::InsufficientMemory { .. })
        ));
        for b in 1..=d.left {
            let bytes = d.workspace(b).unwrap();
            assert_eq!(d.block(bytes).unwrap(), (b, bytes));
            if b > 1 {
                assert_eq!(d.block(bytes - 1).unwrap().0, b - 1);
            }
        }
        assert_eq!(d.block(u64::MAX).unwrap().0, d.left);
        assert!(matches!(
            Dimensions { n: usize::MAX, ..d }.workspace(1),
            Err(Mp2Error::SizeOverflow)
        ));
    }

    #[test]
    fn denominator_error_in_later_block_is_not_lost() {
        let c = DMatrix::identity(4, 4);
        let e = DVector::from_vec(vec![-2., 0., 0., 1.]);
        let eri = integrals(4);
        let spin = Mp2SpinInput {
            mo_coefficients: &c,
            orbital_energies: &e,
            occupied_orbitals: 2,
            frozen_orbitals: 0,
        };
        let d = Dimensions {
            n: 4,
            left: 2,
            right: 2,
            va: 2,
            vb: 2,
        };
        assert!(matches!(
            energy(
                spin,
                spin,
                &eri,
                d.workspace(1).unwrap(),
                Mp2Sector::Rhf,
                &mut |_| {}
            ),
            Err(Mp2Error::NearZeroDenominator { .. })
        ));
    }
}
