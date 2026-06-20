# Roadmap

RustiQ aims to demonstrate that a quantum chemistry code can be modern,
cross-platform, typed, tested, and pleasant to use while keeping the chemistry
implementation in Rust.

## Current Status

- TOML runfiles and XYZ geometries.
- Local and optional online basis-set management.
- Gaussian basis construction.
- RHF and UHF with DIIS.
- RHF-MP2 and UHF-MP2.
- Geometry inspection and transformations.
- Source-located diagnostics for TOML and XYZ inputs.
- Unit tests and CLI sample tests.

## Short Term

- Add a validation suite with reference energies from established packages.
- Document numerical conventions for units, ERIs, normalization, RHF/UHF, and
  MP2.
- Add structured JSON output for automated workflows.
- Add benchmark infrastructure for ERI, SCF, and MP2 paths.
- Improve README examples with validation and reporting outputs.

## Medium Term

- Add memory estimates and clearer limits for larger calculations.
- Improve integral performance with Rust-native screening and batching.
- Add more non-regression tests across molecules, bases, charge states, and spin
  states.
- Add release artifacts for common platforms.

## Research-Grade Requirements

RustiQ should not be treated as research-grade until it has:

- systematic validation against trusted reference implementations;
- documented tolerances and reference datasets;
- clear scientific convention documentation;
- reproducible performance benchmarks;
- stable input and output contracts;
- broader basis-set and method coverage.

## Not Planned Immediately

- Production DFT.
- Geometry optimization and frequencies.
- Large-scale correlated methods.
- GPU acceleration.

These may become interesting later, but the near-term priority is correctness,
validation, documentation, and architecture.
