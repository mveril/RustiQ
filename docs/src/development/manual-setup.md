# Manual setup with Rust and Cargo

## Advantages and limitations

- **Advantages:** builds and calculations run directly on Linux, macOS, or
  Windows. No container or VM is required. This is suitable when native host
  execution, local profiling, or existing cluster build tools matter.
- **Maintenance:** you install native build dependencies and optional tools
  yourself. The repository selects Rust through `rust-toolchain.toml` and Cargo
  dependencies through `Cargo.lock`, but does not pin the entire host environment.
- **Platforms:** native Cargo supports the platforms in the
  [compatibility table](platforms.md), including Windows and Intel macOS.
  Optional PySCF comparisons require Linux, macOS, or WSL2.
- **HPC use:** this route can use the site's existing compiler environment
  without Docker or Nix. Build and run according to cluster policy, using the
  scheduler's assigned resources. A locally built binary still needs a
  compatible target OS, architecture, and runtime dependencies.

## Install and build

Install a C/C++ build toolchain and CMake for native dependencies. On Linux,
use your distribution's development packages; on macOS, install the Xcode
Command Line Tools and CMake. On Windows, install Visual Studio C++ Build Tools
with the Windows SDK, CMake, and the Rust MSVC toolchain.

Install Git and Rust through [`rustup`](https://rustup.rs/), then use Cargo
directly on Linux, macOS, or Windows:

```sh
git clone https://github.com/mveril/RustiQ.git
cd RustiQ
rustup show
cargo build --locked
cargo test
cargo run -- run samples/h2/sto-3g/calculation.toml
```

`rustup show` causes rustup to notice `rust-toolchain.toml` and install the
requested stable toolchain and components if necessary. This route is enough to
build and run RustiQ, but the extra tools and the PySCF reference environment
from the Nix development shell must be installed separately if you need them.
The repository uses [`uv`](https://docs.astral.sh/uv/) for that environment on
Linux, macOS, and WSL2:

```sh
uv run --locked pytest tools/reference
```

PySCF does not support native Windows; use WSL2 for this optional comparison.

## Build for calculations and install the executable

From the repository root, use an optimized build for substantial calculations
or timing comparisons:

```sh
cargo build --release --locked
cargo run --release --locked -- run samples/h2/sto-3g/calculation.toml
```

To install into Cargo's binary directory:

```sh
cargo install --locked --path .
rustiq --help
```

The executable is named `rustiq` (or `rustiq.exe` on Windows); use lowercase on case-sensitive systems. Ensure Cargo's binary
directory is on `PATH`. Keep the checkout for the tutorial's sample files.
