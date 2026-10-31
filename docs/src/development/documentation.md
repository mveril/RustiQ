# Documentation checks

The manual uses mdBook, MathJax for equations, and
[`mdbook-mermaid`](https://github.com/badboy/mdbook-mermaid) for diagrams.
Nix is optional. Run the commands below from the repository root.

## Build the offline manual

The complete documentation build and checks are:

```sh
nix build .#book
```

Open `result/index.html`. The derivation builds the book, bundles pinned
MathJax resources and Mermaid assets, tests supported Rust snippets, and checks
local links and HTML fragments. Its build runs without networking once Nix has
fetched its declared inputs.

Without Nix, install `mdbook` and `mdbook-mermaid` using Cargo or equivalent
packages, and provide Python 3.12 or later with `pytest` installed (see the
[Python tooling setup](manual-setup.md)):

```sh
cargo install mdbook
cargo install mdbook-mermaid
python tools/build-manual.py
mdbook test docs
pytest tools/test_book_links.py tools/test_release_smoke.py
python tools/check-book-links.py docs/book --config docs/book.toml --offline
git diff --check
```

Open `docs/book/index.html`. The builder downloads the pinned MathJax 2.7.9
npm archive, verifies its SHA-256 digest, installs Mermaid resources, and builds
the HTML. It replaces mdBook's CDN loader with relative local MathJax paths,
using SVG equations so reading local files does not require web fonts.
MathJax's license is preserved in `mathjax/LICENSE` in the output.
The local installation follows [MathJax's installation guidance](https://docs.mathjax.org/en/v2.7/installation.html).

To build with networking disabled, supply the already downloaded, matching
[MathJax 2.7.9 npm archive](https://registry.npmjs.org/mathjax/-/mathjax-2.7.9.tgz);
the documentation tools must also be installed:

```sh
python tools/build-manual.py --mathjax-archive /path/to/mathjax-2.7.9.tgz
```

Both routes produce the same relocatable manual. Readers need a browser, but
no checkout, Python, Rust, mdBook, or network connection. External reference
links, repository links, and edit links still require networking.
Generated `docs/book/`, `docs/mermaid.min.js`, and `docs/mermaid-init.js` are
ignored by Git and must not be committed.

## Preview while editing

The Nix `rust` and `full` shells include mdBook and mdbook-mermaid. For a live
preview, enter `nix develop .#rust` if using Nix, then run:

```sh
mdbook-mermaid install docs
mdbook serve docs --open
```

This direct mdBook preview uses its CDN MathJax loader. To inspect the offline
result, stop the server, rerun the offline builder above, and open the generated
`index.html`. Running `mdbook build` or `mdbook serve` directly regenerates the
unmodified CDN loader; it does not produce the distributable offline manual.

## Release integration and coverage

The release and six-target rehearsal workflows build the Nix book once and
supply it as `manual/html/` to every archive build. For a local dist build, first
prepare that directory:

```sh
nix build .#book
mkdir -p manual/html
cp -R result/. manual/html/
dist build --artifacts=local --target x86_64-unknown-linux-gnu
```

Use the target for the native operating system and architecture. The generated
`manual/html/` directory is ignored by Git. The Nix package installs the same book
under `share/doc/rustiq/manual/html` and checks `rustiq docs --print-path` against its
installed index. Cargo builds and published crates do not bundle the book.

Verification coverage is explicit:

- The book build checks chapter structure, sample includes, and Mermaid assets.
- The link checker validates local links, assets, and HTML fragments. Its
  `--offline` mode also rejects remote HTML runtime resources; external
  reference links remain allowed. It does not execute JavaScript or fetch URLs.
- Documentation CI also runs `tools/check-manual-browser.py` against the built
  book in Chromium with networking disabled, checking file navigation, styles,
  rendered SVG equations, and Mermaid diagrams. To reproduce, install
  `playwright==1.64.1` and its Chromium browser in a separate Python environment,
  then run `python tools/check-manual-browser.py docs/book` (or `result`).
- `mdbook test` validates supported Rust blocks; this edition has no Rust
  snippets. It does not execute shell, TOML, XYZ, or Nickel examples.
- CLI integration tests exercise relocated archive and Nix layouts, an
  incomplete manual, browser failures, explicit web selection, and Cargo's
  online-only behavior. Archive rehearsals verify the packaged manual path and
  required MathJax/Mermaid assets after relocation.
- Main CI validates CLI samples, configuration, batches, and PySCF comparisons.
  Documentation CI does not repeat that scientific suite. The
  [first-calculation chapter](../getting-started/first-calculation.md) identifies
  its numerical reference, tolerance, and reproduction command.

The Documentation workflow validates pull requests and supports manual runs.
Publish documentation runs on pushes to `main` and release tags and deploys the validated book
to GitHub Pages. Set the repository's Pages source to **GitHub Actions**.
PR builds have no Pages write permission.

### Published versions

Pushes to `main` publish the current book at `/RustiQ/dev/`. Version tags
publish immutable snapshots at `/RustiQ/vX.Y.Z/`, for example
`/RustiQ/v0.1.0-beta.1/`; a retry fails if the stored
commit or generated content differs. The `gh-pages` branch stores the complete
assembled site, including earlier snapshots. GitHub Pages must remain configured
to deploy using **GitHub Actions**, not from that branch. Publication is
serialized, and pull requests only validate the book.

`/RustiQ/beta/` points to the highest published beta and
`/RustiQ/stable/` to the highest stable release; the stable alias appears only
when a stable release exists. The root redirects to development. On the first publication, existing root HTML
paths become redirects to the corresponding development pages; a page removed
from development will lead to its 404 page. Root assets are removed during this
one-time migration, and future publications do not overlay root files. The first publication also creates redirects for current book pages. To preserve
paths removed from the book before migration, seed `gh-pages` with the existing
published root tree before the first publication.
`/RustiQ/versions.json` lists releases in descending SemVer order, their URLs,
channels and source commit IDs, along with the latest aliases and development URL. URLs in the index are relative to the site root.
The index is static and does not require an API. The offline book remains separate.

For a local assembly test, run `python tools/publish-docs.py result SITE COMMIT
--development`, or replace `--development` with `--version v0.1.0-beta.1`.
`COMMIT` is the full source commit ID and `SITE` is a writable directory. For
example, the index contains entries of this form (the commit is abbreviated
here for readability):

```json
{
  "legacy_redirects": true,
  "development": "dev/",
  "latest_beta": "v0.1.0-beta.1",
  "latest_stable": null,
  "versions": [{"version": "v0.1.0-beta.1", "url": "v0.1.0-beta.1/", "channel": "beta", "commit": "..."}]
}
```

### Repository rename

The current public Pages base remains `https://mveril.github.io/RustiQ/`.
After renaming the repository to `mveril/rustiq`, configure Pages to deploy via
GitHub Actions, update the documentation base URL and the site-specific 404
base, then publish and verify the new site and release links. GitHub repository
redirects do not guarantee redirects for the old Pages URL: arrange and verify
a separate redirect for the old Pages site before relying on existing links.
Previously published binary URLs cannot be changed.
