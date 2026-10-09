# Known supported platforms

RustiQ is written almost entirely in Rust, which makes it portable across a
wide range of operating systems and processor architectures. The CLI and the
`rustiq-core` library are known to work on the platforms below. Automated build
and test checks help maintain that compatibility as the project evolves.

| Operating system | Processor architecture | Available setup |
| --- | --- | --- |
| Linux | x86_64 | Cargo, Nix, or the Dev Container |
| Linux | AArch64 (ARM64) | Cargo, Nix, or the Dev Container |
| Windows | x86_64 | Native Cargo, or a Linux environment through WSL2 or the Dev Container |
| Windows | AArch64 (ARM64) | Native Cargo, or a Linux environment through WSL2 or the Dev Container |
| macOS | Apple Silicon (AArch64) | Cargo, Nix, or the Dev Container |
| macOS | Intel (x86_64) | Cargo or the Dev Container |

Other Rust targets may also work. This table describes known compatibility;
it is not an exhaustive list of every platform on which RustiQ could run.
Native dependencies and optional tools can impose additional requirements.

## Choosing an environment

The Cargo workflow builds RustiQ directly on your operating system. It is
available on all the platforms listed above. The Nix flake provides a prepared
Rust and scientific Python environment for these systems:

- `x86_64-linux`;
- `aarch64-linux`;
- `aarch64-darwin` (Apple Silicon).

The pinned nixpkgs revision no longer supports Intel macOS, so use native
Cargo or the Linux Dev Container on that platform. Native Windows users can also use Cargo; Nix requires a Linux
environment such as WSL2.

The recommended VS Code Dev Container provides a Linux environment through
Docker. It is an option on all the host platforms in the table, including
Linux and Windows ARM64, provided Docker can run Linux containers on the host.
Inside the container, Nix uses `x86_64-linux` or `aarch64-linux` according to
the container architecture, even when the host runs Windows or macOS. See
[Development environments](environments.md) for installation instructions,
Windows troubleshooting, and the configured VS Code extensions.

The optional scientific Python tools have their own platform requirements.
The project configures `uv` for Linux and macOS, and the Nix environment
provides PySCF on every declared Nix system. For PySCF comparisons on Windows,
use WSL2 or the Linux Dev Container.

## How compatibility is checked

The [regular CI workflow](https://github.com/mveril/RustiQ/actions/workflows/ci.yml)
provides ongoing evidence for this support:

- Native Cargo checks cover Linux x86_64, Windows x86_64 and ARM64, and macOS
  Apple Silicon and Intel. They run formatting, Clippy, workspace tests with
  and without the online feature, and standalone core checks.
- Nix checks cover Linux x86_64 and AArch64 and macOS Apple Silicon. They build
  RustiQ, exercise the development shells, and compare reference energies
  against PySCF.
- The Dev Container is built and tested on Linux x86_64, including Cargo tests
  and PySCF comparisons. Separate Windows and macOS host integrations are not
  exercised by these checks.
- The standalone `uv` environment is checked on Linux x86_64.

The [scheduled full Nix workflow](https://github.com/mveril/RustiQ/actions/workflows/nix-full.yml)
also runs all native flake checks on the three declared Nix systems. These
checks validate known configurations; they do not limit the possible
platforms for the Rust application.
