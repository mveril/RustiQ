# Documentation checks

The book uses the standard mdBook theme, search, and MathJax, plus
[`mdbook-mermaid`](https://github.com/badboy/mdbook-mermaid) to render the
TOML/Nickel input-flow diagram in the calculation inputs guide. **Nix is
optional** for building and previewing the manual. You can choose either setup
below.

### With Nix

The `rust` and `full` development shells include both `mdbook` and
`mdbook-mermaid`. Prepare the local Mermaid JavaScript assets once per checkout,
then preview the manual:

```sh
nix develop .#rust
mdbook-mermaid install docs
mdbook serve docs --open
```

For the complete CI-equivalent build and checks, run:

```sh
nix build .#book
```

The Nix derivation installs the Mermaid assets in its temporary source tree,
then builds and validates the manual. No manual asset setup is required for
`nix build .#book`.

### Without Nix

Install the two documentation tools with Cargo, or provide equivalent binaries
on your `PATH`:

```sh
cargo install mdbook
cargo install mdbook-mermaid
mdbook-mermaid install docs
mdbook serve docs --open
```

To build without running the preview server:

```sh
mdbook build docs
mdbook test docs
```

`mdbook-mermaid install docs` copies `mermaid.min.js` and `mermaid-init.js`
into `docs/`; those generated assets are ignored by Git. Run it once after a
fresh checkout, and again when upgrading `mdbook-mermaid` to refresh the
bundled JavaScript. The preprocessor must also be on `PATH` for subsequent
builds. Readers of the published HTML manual need neither Nix nor Rust.

Open `result/index.html` for the generated manual. The Nix derivation runs:

```sh
mdbook-mermaid install docs
mdbook build docs
mdbook test docs
pytest tools/test_book_links.py
python tools/check-book-links.py docs/book --config docs/book.toml
```

It also checks that the generated HTML contains Mermaid markup and that both
Mermaid JavaScript assets are present. Without Nix, the same link checks can
be run with `python -m pytest tools/test_book_links.py` (with `pytest` installed)
and `python tools/check-book-links.py docs/book --config docs/book.toml`.

Verification coverage is deliberately explicit:

- Book build checks chapter structure, sample includes, and Mermaid rendering assets.
- The link checker validates generated local links, assets, and HTML fragments.
  It does not fetch external URLs.
- `mdbook test` validates supported Rust code blocks. This edition has no Rust
  snippets. It does not execute shell, TOML, XYZ, or Nickel examples.
- Main CI validates CLI samples, configuration, batches, and PySCF comparisons.
  Documentation CI does not repeat that scientific suite. The
  [first-calculation chapter](../getting-started/first-calculation.md) identifies
  its sample test, numerical reference, tolerance, and reproduction command.

The Documentation workflow builds and validates pull requests and can also be
run manually. A separate Publish documentation workflow runs on pushes to
`main`, including merges, and deploys to GitHub Pages after successful validation.
The publication job does not appear in pull request checks.
The repository's Pages source must be **GitHub Actions** for this workflow to
deploy the manual. PR builds have no Pages write permission.

When extending the manual, tie executable capabilities to checked-in examples
and automated verification. Numerical claims need a reference and tolerance.
