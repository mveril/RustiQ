# Introduction

RustiQ is an experimental Rust quantum chemistry application and reusable
`rustiq-core` library. This manual serves scientific users, Rust contributors,
and theoretical chemists.

**Do not use RustiQ results for research conclusions without independent
validation against established quantum chemistry packages.**

Start with [installation](getting-started/installation.md), the
[first H₂ calculation](getting-started/first-calculation.md), and
[output interpretation](getting-started/output.md). Contributors can use the
[environment guide](development/environments.md) and
[Rust API documentation](development/contributing.md).

Implemented capabilities include RHF, UHF, MP2, XYZ geometries, Gaussian basis
sets, TOML and native Nickel input, sequential studies, and versioned JSON.
This initial edition covers the first HF calculation. Practical guides and
a complete CLI reference are follow-ups in
[issue #106](https://github.com/mveril/RustiQ/issues/106).

Portable `.rustiq` bundles are available through Rust APIs. The proposed CLI
artifact workflow is tracked in [issue #108](https://github.com/mveril/RustiQ/issues/108);
this edition does not present its commands as shipped behavior.
The [roadmap](https://github.com/mveril/RustiQ/blob/main/ROADMAP.md) describes
plans; see [authoritative references](reference/index.md) for existing contracts.
