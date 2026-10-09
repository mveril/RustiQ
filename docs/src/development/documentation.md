# Documentation checks

The book uses standard mdBook theme, search, and MathJax, without external
preprocessors. Preview it inside `nix develop .#rust`:

```sh
mdbook serve docs --open
```

Run the documentation workflow's checks locally:

```sh
nix build .#book
```

Open `result/index.html` for the generated manual. The derivation runs:

```sh
mdbook build docs
mdbook test docs
python -m unittest discover -s tools -p test_book_links.py
python tools/check-book-links.py docs/book --config docs/book.toml
```

Verification coverage is deliberately explicit:

- Book build checks chapter structure and sample includes.
- The link checker validates generated local links, assets, and HTML fragments.
  It does not fetch external URLs.
- `mdbook test` validates supported Rust code blocks. This edition has no Rust
  snippets. It does not execute shell, TOML, XYZ, or Nickel examples.
- Main CI validates CLI samples, configuration, batches, and PySCF comparisons.
  Documentation CI does not repeat that scientific suite. The
  [first-calculation chapter](../getting-started/first-calculation.md) identifies
  its sample test, numerical reference, tolerance, and reproduction command.

The isolated Documentation workflow builds pull requests and pushes to `main`.
Only successful builds from `main` deploy to GitHub Pages.
Maintainers must select **GitHub Actions** as the repository's Pages source
before the first deployment. PR builds have no Pages write permission.

When extending the manual, tie executable capabilities to checked-in examples
and automated verification. Numerical claims need a reference and tolerance.
