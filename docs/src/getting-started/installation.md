# Installation

## Install a prebuilt release binary

When a release is available, ordinary users can download the archive for their
operating system and CPU from the [GitHub Releases page](https://github.com/mveril/RustiQ/releases).
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

Download the release's `sha256.sum` and all listed assets into one directory.
On Linux or macOS, verify them before extracting:

```sh
sha256sum --check sha256.sum  # Linux
shasum -a 256 --check sha256.sum  # macOS
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
archive only after the checksum comparison succeeds, then run
`./rustiq --help` from its directory on Linux or macOS, or
`.\rustiq.exe --help` from that directory in PowerShell. To use `rustiq` from
any directory, add the directory containing `rustiq` or `rustiq.exe` to `PATH`
and open a new terminal.

The Cargo package is named `rustiq-chem`; the executable is named `rustiq` (or
`rustiq.exe` on Windows).

## Make basis sets available

The archive contains neither sample inputs nor basis data. With network access,
download a basis using `rustiq basis download sto-3g`. This command is available
in online builds and requires network access. To work offline, import a local
Basis Set Exchange JSON file with `rustiq basis import /path/to/basis.json`;
import does not access the network. See the
[basis-set guide](../user-guide/basis-sets.md) and the
[first calculation](first-calculation.md) for a complete binary-only example.

## Build and install from source

Contributors and users building from a checkout need Git, Rust through
[rustup](https://rustup.rs/), a C/C++ build toolchain, and CMake. Follow the
[manual setup](../development/manual-setup.md#install-and-build), then run
these commands from the repository root:

```sh
git clone https://github.com/mveril/RustiQ.git
cd RustiQ
cargo build --locked
```

`cargo build` does not add the executable to `PATH`. To run without installing,
prefix commands with `cargo run --locked --`, for example
`cargo run --locked -- --help`. To install the package executable into Cargo's
binary directory:

```sh
cargo install --locked --path .
rustiq --help
```

The registry package is also named `rustiq-chem`; install from crates.io only
when the relevant version has been published. See the
[development environments](../development/environments.md) page for Nix,
Docker, and other supported contributor setups.
