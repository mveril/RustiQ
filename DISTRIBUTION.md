# RustiQ distribution terms and source access

RustiQ-authored source files are available under **MIT OR Apache-2.0**.
Third-party files keep their upstream licenses and copyrights. The package
manifests describe the RustiQ-authored source; they do not relicense dependencies.

The executable uses **Malachite under LGPL-3.0-only** through Nickel and
**ansi_colours under LGPL-3.0-or-later** through bat. These libraries and their
use are covered by the LGPL. RustiQ's own code remains MIT OR Apache-2.0;
the libraries are not relicensed. The GPLv3 text in `COPYING` accompanies the
LGPL terms, which supplement GPLv3; it does not declare RustiQ as a whole GPL.
`THIRD-PARTY-NOTICES.txt` supplies the LGPL, MPL, data-license and other texts
and upstream notices. The BSD alternative is selected for `bounded-vec-deque`.
This policy also applies to binaries built without the online feature.

## Sources for a distributed executable

Each tagged GitHub release supplies `rustiq-corresponding-source.tar.gz`
alongside its binary archives, at no additional charge. The version-specific
download URL is recorded at the top of `THIRD-PARTY-NOTICES.txt`. Obtain the
source artifact from the **same release as the binary**. It contains the
repository sources, `Cargo.lock`, build scripts, assets, vendored dependencies,
and Cargo configuration for using them without accessing a registry.
Vendored dependencies are the bundled third-party source code in `vendor`.
GitHub's automatic repository source downloads do not contain the vendor tree.

Extract and enter the archive using `tar` on Unix or recent Windows systems:

```sh
tar -xzf rustiq-corresponding-source.tar.gz
cd rustiq-source
```

Install Rust, a native
C/C++ toolchain and CMake as described in the manual's Cargo setup guide.
Build the executable using:

```sh
cargo build --offline --locked --profile dist --bin rustiq
```

The result is `target/dist/rustiq` (`target/dist/rustiq.exe` on Windows).
This rebuild enables the default online feature. An offline application build
adds `--no-default-features`; the build itself stays offline in both cases.
This is a reconstruction procedure, not a guarantee of byte-identical binaries.
`SOURCE-INFO.txt` identifies the repository commit and lockfile.

To replace an LGPL component, copy its directory from `vendor` to an editable
directory and add a path override to the existing `[patch.crates-io]` table
in the root `Cargo.toml`.
For example, after copying `vendor/malachite-base-0.9.2` to
`modified/malachite-base`, add:

```toml
# Add this entry to the existing [patch.crates-io] table.
malachite-base = { path = "modified/malachite-base" }
```

Then rebuild with `cargo build --offline --profile dist --bin rustiq`.
Omit `--locked` for this modified build because the patch changes the lockfile.
The replacement must satisfy the required package version and API, or you
must adjust its dependents too. Keep the upstream license and document your
modifications. Vendored files
have integrity checks, so use a path patch instead of editing them in place.

## Redistributing or packaging RustiQ

Include `COPYING`, `LICENSE-MIT`, `LICENSE-APACHE`, `DISTRIBUTION.md` and
`THIRD-PARTY-NOTICES.txt` with each executable archive or installed package.
Preserve the upstream notices, including bundled native code and assets.
Package metadata may describe RustiQ-authored code as MIT OR Apache-2.0.
It must also preserve the notices and terms for the bundled LGPL, MPL and other
components; the application license does not replace those terms.

When serving binaries for download, give recipients equally convenient,
no-charge access to the exact corresponding sources on the same download
page. If the sources are on another server, put clear directions next to the
binary and keep that source access available for as long as needed to meet
LGPLv3 section 4(d)(0), which refers to GPLv3 section 6. Preserve the source
link in the installed documentation.
If you rebuild or patch the application, supply corresponding sources for
your own build, including packaging scripts and modifications. Media,
devices and other distribution methods require the applicable GPLv3 section
6 arrangements, including installation information when required.

The upstream LGPL and MPL sources and any modifications must remain
available under their applicable licenses. Do not add terms that prevent
modification, replacement of LGPL components, or reverse engineering to debug
those modifications. Normal command execution does not require a credits
banner; the accompanying documents carry the notices.

## Dependency review

`cargo-deny` restricts additional license approvals to reviewed crate versions.
`cargo-about` generates license texts; `tools/distribution.py` adds separate
upstream notices and records the lockfile and input hashes. Identical upstream
texts are included once and referenced by each package that supplies them.
`cargo vendor` gathers dependency sources; `git archive` supplies the committed
project tree; cargo-dist publishes the resulting archive.
CI checks the package inventory and generated-file integrity, not every license
clause. Supplemental upstream notices and their provenance
are retained in `licenses/extra-notices.txt`, including notices missing from
published crate archives. Review these when updating the affected crates.

RustiQ retains transitive dependencies with informational maintenance
advisories for bincode, unsafe-libyaml and yaml-rust. Their specific advisory
IDs and reasons are recorded in `deny.toml`; this accepts maintenance risk, not
future vulnerabilities. Rustls is updated to its fixed patch release. Nickel
0.19 currently depends on `imbl-sized-chunks` 0.1, which is affected by
RUSTSEC-2026-0292. RustiQ uses Nickel's published dependency without a local
backport; `deny.toml` records this accepted upstream risk until Nickel releases
a compatible update. That dependency graph also includes the unmaintained
`bitmaps` 3.2.1 crate (RUSTSEC-2026-0247), for which no safe upstream upgrade is
available. These upstream risks are listed separately in `deny.toml`.

Authoritative terms: [GPLv3](https://www.gnu.org/licenses/gpl-3.0.html),
[LGPLv3](https://www.gnu.org/licenses/lgpl-3.0.html),
[MPL 2.0](https://www.mozilla.org/en-US/MPL/2.0/), and
[CDLA-Permissive-2.0](https://cdla.dev/permissive-2-0/).
