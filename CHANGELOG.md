# Changelog

All notable changes to RustiQ are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

The following capabilities are intended for the first public beta, `v0.1.0-beta.1`.

### Added

- Restricted Hartree–Fock (RHF) and unrestricted Hartree–Fock (UHF), with
  automatic reference selection from electron count and spin multiplicity.
- RHF-MP2 and UHF-MP2 correlation energies from converged HF references,
  optional frozen occupied orbitals, and blocked integral transformations with
  a configurable workspace budget.
- Configurable SCF iteration limits, convergence thresholds, initial guesses,
  and DIIS acceleration, with optimal damping safeguards and convergence,
  overlap-rank, and UHF spin-contamination diagnostics.
- Gaussian basis-set import, listing, removal, and optional online downloads
  from Basis Set Exchange, with Cartesian and supported spherical functions.
- XYZ geometry input with Angstrom or Bohr calculation units, molecular charge
  and multiplicity, plus geometry inspection, translation, rotation, centering,
  and orientation tools.
- TOML single-calculation inputs and native Nickel configurations with shared
  defaults, validation, imports, and composition. Nickel batches execute
  sequentially and retain ordered per-calculation outcomes.
- The `rustiq` CLI for input generation and calculation execution, versioned
  JSON calculation and batch output, and exit codes for convergence outcomes.
- Optional local caching of atomic-orbital electron-repulsion integrals
  (AO ERIs), with cache listing and removal commands.
- Portable `.rustiq` bundles containing one or more calculation input snapshots,
  basis data, and supported AO ERIs; bundle creation, inspection, integrity
  verification, read-only reuse, and direct reruns without original input files.
- A reusable `rustiq-chem-core` Rust library with typed calculation preparation,
  execution, and HF solution APIs.
- Binary release packaging for Linux, macOS, and Windows on x86_64 and ARM64,
  with SHA-256 checksums, license notices, and corresponding dependency sources;
  installation from a checkout through Cargo and Nix packages.
- An offline HTML manual bundled with binary archives and Nix packages,
  accessible through `rustiq docs`; Cargo-only installations use the online
  manual. Manpage and shell-completion generators are enabled by default.

### Known Limitations

- RustiQ is experimental quantum chemistry software. Scientific validation
  covers selected reference cases and does not guarantee research-grade
  correctness; independently validate results against established packages.
- Electronic-structure methods are limited to Hartree–Fock and MP2. DFT,
  geometry optimization, and vibrational frequencies are not implemented.
- Effective core potentials and spherical Gaussian functions above d angular
  momentum are unsupported. Basis availability does not imply validation.
- Portable `.rustiq` bundles do not persist converged HF solutions, partial SCF
  states, DIIS history, or MP2 results. Archive reruns start fresh SCF
  calculations, potentially reusing compatible stored AO integrals.
- The MP2 workspace budget does not bound total process memory; AO integral
  storage can still grow rapidly with basis size.

[Unreleased]: https://github.com/mveril/RustiQ/commits/main/
