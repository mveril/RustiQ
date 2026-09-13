//! Hartree--Fock self-consistent-field implementation.
//!
//! For RHF, the closed-shell AO density and Fock matrices are
//!
//! \[
//! P_{\mu\nu} = 2\sum_i^{\mathrm{occ}} C_{\mu i} C_{\nu i},
//! \qquad
//! F_{\mu\nu} = H_{\mu\nu}^{\mathrm{core}}
//! + \sum_{\lambda\sigma} P_{\lambda\sigma}
//! \left[(\mu\nu\mid\lambda\sigma)
//! - \tfrac{1}{2}(\mu\sigma\mid\lambda\nu)\right].
//! \]
//!
//! The electronic energy is evaluated as
//!
//! \[
//! E_{\mathrm{elec}} = \tfrac{1}{2}
//! \sum_{\mu\nu} P_{\mu\nu}
//! \left(H_{\mu\nu}^{\mathrm{core}} + F_{\mu\nu}\right),
//! \qquad E_{\mathrm{total}} = E_{\mathrm{elec}} + E_{\mathrm{nuc}}.
//! \]
//!
//! UHF uses separate \(\alpha\) and \(\beta\) densities. For spin \(s\), its
//! Fock matrix has Coulomb contributions from \(P^\alpha + P^\beta\) and exchange
//! contributions only from \(P^s\). Iterations solve the Roothaan--Hall equation
//! \(FC = SC\varepsilon\); convergence requires both the energy change and the
//! Frobenius norm of \(FPS - SPF\) to be below the configured threshold.

pub(crate) mod component;
pub(crate) mod core;
pub(crate) mod density_guess;
pub(crate) mod diis;
pub(crate) mod numerical_error;
pub(crate) mod orthogonalization;
pub(crate) mod scf;
pub(crate) mod scf_energy_details;
pub(crate) mod scf_iteration;
pub(crate) mod scf_result;
pub(crate) mod scf_setup;
pub(crate) mod uhf;
