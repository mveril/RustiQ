# Introduction

RustiQ is an experimental Rust quantum chemistry application and reusable
`rustiq-chem-core` library. This manual serves scientific users, Rust contributors,
and theoretical chemists.

**Do not use RustiQ results for research conclusions without independent
validation against established quantum chemistry packages.**

## Where should I start?

- **Scientific users:** [Installation](getting-started/installation.md) →
  [First H₂ calculation](getting-started/first-calculation.md) →
  [Interpreting output](getting-started/output.md). These explain how to run
  the software without writing Rust. Read [scientific limitations](scientific-scope.md)
  before using the results.
- **Rust developers and architects:** [Development environments](development/environments.md)
  → [Contributing, architecture, and the Rust API](development/contributing.md)
  → [Documentation checks](development/documentation.md). Start with the
  calculation lifecycle to see how input, scientific code, and reporting fit together.
- **Theoretical chemists:** [First calculation](getting-started/first-calculation.md)
  → [Reference comparisons](scientific-scope.md#reference-comparisons) →
  [Scientific limitations](scientific-scope.md#major-limitations). These identify
  tested evidence and current assumptions without requiring development tools.

The [glossary](glossary.md) explains unfamiliar scientific and software terms.
[Existing workflow notes](reference/existing-workflows.md) preserve useful
operational guidance while the complete guides are developed.

Implemented capabilities include RHF, UHF, MP2, XYZ geometries, Gaussian basis
sets, TOML and native Nickel input, sequential studies, and versioned JSON.
The [practical user guides](user-guide/geometry.md) now cover these capabilities,
including HF/MP2 calculations, Nickel studies, and JSON output. A comprehensive
CLI reference remains planned for PR C1 in
[issue #106](https://github.com/mveril/RustiQ/issues/106).

Portable `.rustiq` bundles are available through Rust APIs. The proposed CLI
artifact workflow is tracked in [issue #108](https://github.com/mveril/RustiQ/issues/108);
this edition does not present its commands as shipped behavior.
The [authoritative references](reference/index.md) page links to the roadmap
for plans and to the files that define existing contracts.
