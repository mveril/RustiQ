# Installation

For a guided setup with the tools prepared for you, start with
[Development environments](../development/environments.md). That chapter
introduces VS Code, Docker, Dev Containers, Nix, and WSL before explaining how
to use them. See [Known supported platforms](../development/platforms.md) to
choose a setup for your computer.

The instructions below describe the alternative of installing the build tools
yourself. Rust is the language used to implement RustiQ; Cargo, included with
Rust, compiles the source code into the executable that runs your calculations.

Install Git and stable Rust through [rustup](https://rustup.rs/).
Native dependencies also need a C/C++ build toolchain and CMake.
On Windows, use the MSVC toolchain and Visual Studio C++ Build Tools.
Cargo CI covers Linux, Windows, Apple Silicon macOS, and Intel macOS.

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
