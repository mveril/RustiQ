//! Hartree--Fock self-consistent-field implementation.
//!
//! For RHF, the closed-shell AO density and Fock matrices are
//!
//! ```text
//! P_μν = 2 Σ_i∈occ C_μi C_νi
//! F_μν = H^core_μν + Σ_λσ P_λσ [(μν|λσ) − ½(μσ|λν)]
//! ```
//!
//! The electronic energy is evaluated as
//!
//! ```text
//! E_elec  = ½ Σ_μν P_μν (H^core_μν + F_μν)
//! E_total = E_elec + E_nuc
//! ```
//!
//! UHF uses separate α and β densities. For spin s, its Fock matrix has Coulomb
//! contributions from Pᵅ + Pᵝ and exchange contributions only from Pˢ. Iterations
//! solve the Roothaan--Hall equation F C = S C ε; convergence requires both the
//! energy change and the Frobenius norm of F P S − S P F to be below the configured
//! threshold.

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
