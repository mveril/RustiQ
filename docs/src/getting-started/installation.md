# Installation

After a release promotion completes, RustiQ is distributed through a prebuilt
archive on GitHub Releases, the Cargo registry, and the Nix flake. The three
publications run independently, so use the channel where the release is listed.
They are all built from the same release tag.

| Method | Choose it when | Platforms and manual |
| --- | --- | --- |
| [GitHub Releases archive](#install-a-prebuilt-release-binary) | You want a runnable executable without installing Rust or Nix. | The six target platforms listed below; includes the offline manual. |
| [Cargo](#install-with-cargo) | You already use Rust and Cargo, or want Cargo to build RustiQ locally. | Native Cargo-supported platforms; uses the online manual. |
| [Nix](installation-nix.md) | You already use Nix or want declarative installation and a pinned package environment. | Linux x86_64/ARM64 and Apple Silicon macOS; includes the offline manual. |

## Install a prebuilt release binary

Download the archive for your operating system and CPU from the
[GitHub Releases page](https://github.com/mveril/RustiQ/releases).
Archives are named `rustiq-chem-<target>.tar.gz` on Unix and
`rustiq-chem-<target>.zip` on Windows. Supported targets are
`x86_64-unknown-linux-gnu` (Linux Intel/AMD),
`aarch64-unknown-linux-gnu` (Linux ARM64), `x86_64-apple-darwin` (Intel
macOS), `aarch64-apple-darwin` (Apple Silicon),
`x86_64-pc-windows-msvc` (Windows Intel/AMD), and
`aarch64-pc-windows-msvc` (Windows ARM64). Linux GNU archives require a
compatible glibc runtime.

RustiQ-authored code is MIT OR Apache-2.0. Executables include LGPL libraries,
third-party notices and a matching corresponding-source archive on the same
release page. See [licenses and redistribution](../development/licensing.md)
for source access, rebuilding and package redistribution requirements.

## Verify and run the executable

Download the release's `sha256.sum` beside the archive chosen for your platform.
You do not need the archives for other platforms. Copy only the matching line
from `sha256.sum` into a new text file named `selected.sha256`, preserving both
the hash and filename. On Linux or macOS, verify it before extracting:

```sh
sha256sum --check selected.sha256  # Linux
shasum -a 256 --check selected.sha256  # macOS
```

On Windows, use PowerShell to compare the downloaded archive's computed hash
with its matching entry in `sha256.sum`:

```powershell
$archive = "rustiq-chem-x86_64-pc-windows-msvc.zip"
$entries = @(Select-String -Path .\sha256.sum -SimpleMatch $archive)
if ($entries.Count -ne 1) { throw "Expected one checksum entry for $archive" }
$expected = ($entries[0].Line -split '\s+')[0]
$actual = (Get-FileHash ".\$archive" -Algorithm SHA256).Hash
if ($actual -ine $expected) { throw "SHA-256 mismatch for $archive" }
Write-Host "SHA-256 verified: $archive"
```

The [release guide](../development/releasing.md#binary-releases) has the full
platform-specific verification instructions. For Windows ARM64, replace the
archive filename with `rustiq-chem-aarch64-pc-windows-msvc.zip`. Extract the
archive only after the checksum comparison succeeds. From the directory holding
the download, extract it with one of the following commands:

```sh
tar -xzf rustiq-chem-<target>.tar.gz  # Linux or macOS
```

```powershell
Expand-Archive rustiq-chem-<target>.zip -DestinationPath .\rustiq-chem
```

Open a terminal in the extracted directory and run `./rustiq --help` on Linux
or macOS, or `.\rustiq.exe --help` in PowerShell. This works without changing
`PATH`. To run `rustiq` from any directory, add the directory containing
`rustiq` or `rustiq.exe` to `PATH` and open a new terminal.

The Cargo package is named `rustiq-chem`; the executable is named `rustiq` (or
`rustiq.exe` on Windows).

## Install with Cargo

Install the released Cargo package from crates.io from any directory. No source
checkout is needed. This builds RustiQ from source and requires Rust and Cargo,
a C/C++ toolchain, and CMake; see [manual setup](../development/manual-setup.md#install-and-build)
for platform prerequisites, but do not follow that page's checkout commands
unless you intend to contribute.

```sh
cargo install --locked rustiq-chem --version '=0.1.0-beta.1'
rustiq --help
```

Ensure Cargo's binary directory is on `PATH`. Cargo-only installations use the
online manual, as described below.

## Install with Nix

See [installing with Nix](installation-nix.md) for the supported platforms,
profile and declarative installation, and CPU-specific builds. That page uses
the repository's default branch, `main`, which contains released versions only.

## Consult the manual

Release archives contain an offline HTML manual in `manual/html/` beside the
executable. Keep that directory with the executable when moving the extracted
installation. The Nix package installs the same offline manual under
`share/doc/rustiq/manual/html`. Navigation, styling, equations, and diagrams use local
resources; external reference links require network access.

```sh
rustiq docs
rustiq docs --web
rustiq docs --print-path
```

`rustiq docs` opens the installed manual in the default browser using a browser-specific
launcher, independently of HTML file associations. `--web` forces
the online manual for the compiled release version; development builds use
[the development manual](https://mveril.github.io/RustiQ/dev/). The choice is
made from the Cargo package version without a network lookup. Versions with a
complete `dev` prerelease identifier, such as `0.1.0-alpha.1.dev`, select `/dev/`.
Other versions, including alpha and beta releases, select `/vVERSION/`.
This applies equally to Cargo, Nix, and release archives, without packaging flags.
A release-version source build selects its versioned manual even before publication.
`--print-path` prints the
absolute local index path without starting a browser; combine it with `--web`
to print the online URL instead. On headless machines, copy the manual tree to
another machine and open its `index.html`, or use the printed URL in a browser
with network access.

Cargo-only installations have no bundled manual: `rustiq docs` opens the online
manual and `--print-path` prints its URL. RustiQ does not download the manual.
An existing manual directory with a missing index produces an error; restore
the complete installation or use `--web`. If the whole manual directory has
been removed, RustiQ cannot distinguish the installation from a Cargo install
and uses the online manual.

If browser launching fails, the diagnostic includes the target and the
`--print-path` route. RustiQ uses the operating system's default browser opener.
PDF export is future work and `--pdf` is not supported.

## After installation: obtain a basis and run a calculation

Run `rustiq --version` to confirm the installation. Installed release packages
do not include sample inputs or basis data. With network access, download a
basis using `rustiq basis download sto-3g`. This command is available in online
builds and requires network access. To work offline, import a local Basis Set
Exchange JSON file with `rustiq basis import /path/to/basis.json`; import does
not access the network. The [first calculation](first-calculation.md) works
with an archive, Cargo, or Nix installation. See the
[basis-set guide](../user-guide/basis-sets.md) for basis-data details.

## Build and install from source

Contributors and users building from a checkout need Git, Rust through
[rustup](https://rustup.rs/), a C/C++ build toolchain, and CMake. Clone from a
parent directory, then run Cargo commands from the checkout root. For complete
prerequisite and platform details, see the [manual setup](../development/manual-setup.md#install-and-build).

```sh
git clone --branch develop --single-branch https://github.com/mveril/RustiQ.git
cd RustiQ
cargo build --locked
```

The default branch, `main`, is reserved for released versions. Use `develop`
for an active development checkout; a checkout intended to reproduce a release
should instead use its release tag.

For example, to reproduce the `0.1.0-beta.1` release source, clone its tag:

```sh
git clone --branch v0.1.0-beta.1 --single-branch https://github.com/mveril/RustiQ.git
```

`cargo build` does not add the executable to `PATH`. To run without installing,
prefix commands with `cargo run --locked --`, for example
`cargo run --locked -- --help`. To install the package executable into Cargo's
binary directory:

```sh
cargo install --locked --path .
rustiq --help
```

See the [development environments](../development/environments.md) page for Nix,
Docker, and other supported contributor setups.
