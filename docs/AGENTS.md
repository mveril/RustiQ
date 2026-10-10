# Documentation Guidelines

These instructions apply to `docs/` in addition to the repository guidelines.
The root book-maintenance requirement applies to changes throughout the workspace.

## Structure and Audience

- Edit manual sources in `docs/src/` and configuration in `docs/book.toml`. Treat `docs/book/` as generated output; do not edit it manually or commit generated builds.
- Register new chapters in `docs/src/SUMMARY.md`. Preserve working relative links and section anchors when moving or renaming content.
- Write in English for scientific users, theoretical chemists, and Rust contributors. Explain unfamiliar terms, units, and prerequisites at first use, and maintain `docs/src/glossary.md` when adding shared vocabulary.
- Keep getting-started instructions runnable from a stated working directory. Explain installed-binary names, path resolution, basis availability, online/offline requirements, and platform differences where relevant.
- Document shipped behavior. Identify proposed capabilities as future work and link to the roadmap or issue rather than presenting planned commands as available.

## Scientific Requirements

- State method assumptions and limits alongside capabilities. Distinguish SCF convergence, implementation agreement, basis adequacy, and physical validity. Preserve the project's experimental status and independent-validation guidance.
- Specify units and energy definitions. Distinguish electronic, nuclear repulsion, total HF, MP2 correlation, and total MP2 energies when discussing results.
- Explain numerical diagnostics and thresholds precisely, including their defaults, mathematical meaning, and effect on the calculation. Cover residuals, overlap rank reduction, spin contamination, screening, and reference-specific frozen-orbital counting when affected.
- Support numerical claims with a checked-in input, the relevant automated check or independent reference, an explicit tolerance, and a reproduction command. Distinguish rounded terminal output from full-precision comparisons and describe the limits of the validated cases.
- Link scientific claims to primary literature or authoritative method documentation where appropriate. Explain limitations such as strong correlation and small MP2 denominators; numerical safeguards do not establish physical reliability.
- Prefer mdBook `{{#include ...}}` directives for checked-in sample inputs over duplicated configurations. Preserve existing samples and fixtures unless the requested behavior requires changing them.

## Architecture and Contracts

- Keep the distinction between CLI parsing/reporting and scientific library responsibilities clear. Update lifecycle descriptions when preparation, execution, typed outcomes, ownership, errors, or reuse behavior changes.
- Explain when expensive numerical data is created, retained, shared, recomputed, or reused. Distinguish prepared inputs, HF solutions, disposable AO ERI caches, and portable `.rustiq` bundles.
- Describe resource budgets by what they actually cover. Do not describe the MP2 workspace budget as a total process-memory limit or imply that reusable preparation automatically caches integrals.
- Keep rustdoc as the public API reference and link runnable examples and API tests. Explain architecture in the book without duplicating item-level API documentation.
- Use `docs/src/reference/index.md` to link authoritative contracts, including Nickel defaults and validation, versioned JSON schemas, and the persistence specification. Update those contracts when required rather than redefining them only in prose.

## Verification

Run the documentation workflow's complete check for manual changes:

```sh
nix build .#book
```

When using the tools directly, run the equivalent checks from the repository root:

```sh
mdbook build docs
mdbook test docs
pytest tools/test_book_links.py
python tools/check-book-links.py docs/book --config docs/book.toml
git diff --check
```

The standard book enables MathJax and uses no external preprocessors. Keep
diagrams compatible with that build, for example plain text or checked-in SVG;
do not assume Mermaid blocks render without adding and documenting support.

The local link checker checks generated links, assets, and HTML fragments; it
does not fetch external URLs. `mdbook test` checks supported Rust snippets;
it does not execute shell, TOML, XYZ, or Nickel examples. Validate changed
executable guidance with the relevant CLI/API tests or reference comparisons.
Run scientific comparisons when numerical behavior or numerical claims change,
not merely because prose was reformatted. Report the checks actually run and
any remaining verification limits.
