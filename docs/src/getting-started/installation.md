# Installation

Compare [Docker, Nix, and manual installation](../development/environments.md)
before choosing your setup. Each has a dedicated page with prerequisites,
commands, advantages, and limitations. Docker always runs the supplied
workspace under Linux, including on Windows and macOS hosts. For native host
execution, use [manual installation](../development/manual-setup.md), or
[Nix](../development/nix-setup.md) on a supported platform.
See [known supported platforms](../development/platforms.md) for details.

The quick commands below use the manual setup. Install Git, Rust through
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
RustiQ --help
```

The executable is named `RustiQ`; case matters on Linux. Ensure Cargo's binary
directory is on `PATH`. The tutorial still needs the checked-in samples.

Online basis downloads are enabled by default. The
[first calculation](first-calculation.md) imports a repository fixture and
requires no basis download. It also works with `--no-default-features`.
