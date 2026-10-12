# Installation

Compare [Docker, Nix, and manual installation](../development/environments.md)
before choosing your setup. Each has a dedicated page with prerequisites,
commands, advantages, and limitations. Docker always runs the supplied
workspace under Linux, including on Windows and macOS hosts. For native host
execution, use [manual installation](../development/manual-setup.md), or
[Nix](../development/nix-setup.md) on a supported platform.
See [known supported platforms](../development/platforms.md) for details.

The source-build commands below use the manual setup. Install Git, Rust through
[rustup](https://rustup.rs/), a C/C++ build toolchain, and CMake as described in
the [manual setup page](../development/manual-setup.md#install-and-build).

Build from source:

```sh
git clone https://github.com/mveril/RustiQ.git
cd RustiQ
rustup show
cargo build --locked
```

The checked-in `rust-toolchain.toml` selects the Rust toolchain.
Run this guide's commands from the repository root.

Alternatively, enter `nix develop .#rust` after cloning. The flake declares
x86_64 Linux, AArch64 Linux, and Apple Silicon macOS. Windows users can use
WSL2 or the VS Code Dev Container for the Linux environment.
See [development environments](../development/environments.md) for setup and
troubleshooting.

To install the executable into Cargo's binary directory:

```sh
cargo install --locked --path .
rustiq --help
```

This command installs from the checked-out source. Once the current package
prerelease has been published to crates.io, install it from the registry with
`cargo install --locked rustiq-chem --version 0.1.0-alpha.1`. For later stable
releases, omit `--version` to install the latest stable version. Tagged binary
archives and verification instructions are described in
[Releasing RustiQ](../development/releasing.md).

For calculations without a source checkout or Rust installation, download the
archive matching your operating system and CPU from the [GitHub Releases page](https://github.com/mveril/RustiQ/releases).
The archive contains the executable, but not sample inputs or basis data. With
network access, install a basis into your user data directory with
`rustiq basis download sto-3g`. If you already have a Basis Set Exchange JSON
file, import it with `rustiq basis import /path/to/basis.json`; this works with
the release executable and requires no Cargo installation. See the
[basis-set guide](../user-guide/basis-sets.md) for details.

When a tagged binary release is available, download the archive matching your
operating system and CPU from the [GitHub Releases page](https://github.com/mveril/RustiQ/releases).
Unix archives are named `rustiq-chem-<target>.tar.gz`; Windows archives use
`rustiq-chem-<target>.zip`. The targets are `x86_64-unknown-linux-gnu`
(64-bit Intel/AMD Linux), `aarch64-unknown-linux-gnu` (64-bit ARM Linux),
`x86_64-apple-darwin` (Intel macOS), `aarch64-apple-darwin` (Apple Silicon),
`x86_64-pc-windows-msvc` (64-bit Intel/AMD Windows), and
`aarch64-pc-windows-msvc` (Windows on ARM). Verify the archive with its
`.sha256` file or the
release's `sha256.sum` manifest as described in
[Releasing RustiQ](../development/releasing.md), then extract it and put
`rustiq` (or `rustiq.exe` on Windows) in a directory on `PATH`. Prebuilt
binaries include online basis commands, but do not contain basis data. Download
or import the basis required by the calculation as described above.

The executable is named `rustiq` (or `rustiq.exe` on Windows); use lowercase
on case-sensitive systems. Ensure the directory containing it is on `PATH`.
Sample-based tutorials require a source checkout.

The [first calculation](first-calculation.md) includes a binary-only path that
creates its input locally and downloads STO-3G, plus the source-checkout
fixture workflow for offline use.
