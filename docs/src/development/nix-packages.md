# Nix packages and shells

The packages below are declared in [`flake.nix`](https://github.com/mveril/RustiQ/blob/main/flake.nix). All four
shells share Rust build essentials. `rust` adds Rust development tools;
`mini-pyscf` adds the Python reference-test environment to the minimal Rust
shell; `full` combines both extensions. `nix develop` selects `full` by default.

Use `nix develop .#<shell>` to enter a named shell. The `direnv` setup can
select the same names with `RUSTIQ_DEV_SHELL`; see [Development
environments](environments.md).

## Rust build essentials

| Package                                   | Purpose                                                                                     | Shells       |
| ----------------------------------------- | ------------------------------------------------------------------------------------------- | ------------ |
| Rust toolchain from `rust-toolchain.toml` | `rustc`, Cargo, Clippy, rustfmt, and Rust source components pinned to the project toolchain | All          |
| `cmake`                                   | Builds native dependencies that use CMake                                                   | All          |
| `pkg-config`                              | Helps native builds find installed libraries                                                | All          |
| `libiconv` (macOS only)                   | Provides character conversion support needed by native builds on macOS                      | All on macOS |

## Rust development tools

| Package                         | Purpose                                                                 | Shells                  |
| ------------------------------- | ----------------------------------------------------------------------- | ----------------------- |
| `rust-analyzer`                 | Rust language server for editor completion, diagnostics, and navigation | `rust`, `full`          |
| `cargo-nextest`                 | Runs Rust tests with improved reporting and test isolation              | `rust`, `full`          |
| `cargo-deny`                    | Checks dependency licenses, advisories, sources, and duplicate crates   | `rust`, `full`          |
| `cargo-about`                   | Generates the third-party license notices used in distributions         | `rust`, `full`          |
| `cargo-llvm-cov`                | Measures Rust test code coverage using LLVM instrumentation             | `rust`, `full`          |
| `cargo-criterion`               | Runs Criterion benchmarks through Cargo                                 | `rust`, `full`          |
| `cargo-expand`                  | Displays macro-expanded Rust code                                       | `rust`, `full`          |
| `cargo-edit`                    | Adds, removes, and upgrades dependencies from the command line          | `rust`, `full`          |
| `cargo-dist` (`dist`)           | Builds release archives, installers, and checksums                      | `rust`, `full`          |
| `cargo-info` (`cargo info`)     | Displays crate metadata from registries                                 | `rust`, `full`          |
| `bacon`                         | Watches files and reruns selected Cargo checks                          | `rust`, `full`          |
| `cargo-watch`                   | Runs Cargo commands when files change                                   | `rust`, `full`          |
| `git`                           | Source control commands                                                 | `rust`, `full`          |
| `hyperfine`                     | Repeats and compares command-line benchmarks                            | `rust`, `full`          |
| `jq`                            | Reads and transforms JSON from the command line                         | `rust`, `full`          |
| `mdbook`                        | Builds and previews this manual                                         | `rust`, `full`          |
| `mdbook-mermaid`                | Preprocesses Mermaid diagrams in the manual                             | `rust`, `full`          |
| `nixd`                          | Nix language server for editor support                                  | `rust`, `full`          |
| `nixfmt`                        | Formats Nix expressions                                                 | `rust`, `full`          |
| `nickel`                        | Parses and evaluates Nickel configuration files                         | `rust`, `full`          |
| `nls`                           | Nickel language server for editor diagnostics and completion            | `rust`, `full`          |
| `ripgrep` (`rg`)                | Fast recursive text search                                              | `rust`, `full`          |
| `time`                          | Reports elapsed time and resource usage for commands                    | `rust`, `full`          |
| `clang` (Linux only)            | C/C++ compiler used by profiling and native development tools           | `rust`, `full` on Linux |
| `cargo-flamegraph` (Linux only) | Profiles Rust programs and creates flamegraphs                          | `rust`, `full` on Linux |
| `gdb` (Linux only)              | Command-line debugger                                                   | `rust`, `full` on Linux |
| `inferno` (Linux only)          | Converts profiler output into flamegraphs                               | `rust`, `full` on Linux |
| `perf` (Linux only)             | Linux performance-counter profiler                                      | `rust`, `full` on Linux |
| `samply` (macOS only)           | Sampling profiler for macOS                                             | `rust`, `full` on macOS |

## Python and scientific packages

Python dependencies are described by `pyproject.toml` and pinned in `uv.lock`.
Nix builds the Python environments from those files. Python is 3.14 in this
flake.

| Package or group | Purpose                                                                                    | Shells               |
| ---------------- | ------------------------------------------------------------------------------------------ | -------------------- |
| `uv`             | Python project and dependency manager for workflows outside the prebuilt shell environment | `full`               |
| `pyscf`          | Independent quantum-chemistry implementation used for reference calculations               | `mini-pyscf`, `full` |
| `numpy`          | Array and numerical operations used in tests and scientific Python tools                   | `mini-pyscf`, `full` |
| `pytest`         | Runs the PySCF reference test suite                                                        | `mini-pyscf`, `full` |
| `scipy`          | Scientific algorithms used by optional analysis and development tools                      | `full`               |
| `matplotlib`     | Creates plots for scientific analysis                                                      | `full`               |
| `ipykernel`      | Connects Python environments to Jupyter kernels                                            | `full`               |
| `jupyterlab`     | Interactive notebook environment                                                           | `full`               |
| `ruff`           | Lints and formats Python code                                                              | `full`               |

`mini-pyscf` uses the Python `test` dependency group; `full` includes the
`default` group, which combines `dev`, `test`, and `linting`. The helper
`nix run .#pyscf-check -- ...` runs the reference tests against the packaged
RustiQ binary. For example:

```sh
nix run .#pyscf-check -- -q
```

## Other flake outputs

The flake also exposes `nix build .#book` to build this manual,
`nix build .#cargo-artifacts` for cached Cargo dependency artifacts, and
`nix build .#pyscf-environment` for the base PySCF environment. These are build
outputs rather than development shells. The default package is the RustiQ CLI
binary, available through `nix build`.

## Installed manual and completions

The default package includes manual pages for `man rustiq` and its
subcommands, such as `man rustiq-artifact-inspect`. Every page links to the
online book. The package also includes the complete offline HTML manual under
`share/doc/rustiq/manual/html`, including local equation and diagram resources. From the
repository root:

```sh
nix build
./result/bin/rustiq docs
./result/bin/rustiq docs --print-path
```

Use `--web` to open the online manual. Cargo-only installations use the online
manual by default; see [installation](../getting-started/installation.md#consult-the-manual).

The package also installs Bash, Zsh, and Fish completions in the standard
Nix locations. Shell discovery depends on your Nix profile and shell setup.

Nushell, PowerShell, and Elvish scripts are provided under
`share/rustiq/completions/`. From the repository root, build the package with
`nix build`, then load the script in the matching shell:

```nu
use ./result/share/rustiq/completions/rustiq.nu *
```

```powershell
. ./result/share/rustiq/completions/rustiq.ps1
```

```elvish
source ./result/share/rustiq/completions/rustiq.elv
```

For persistent configuration, use the corresponding installed profile path
instead of a checkout's `result` link. PowerShell support registers native
argument completion only.

Nix generates the files with a separate build tool and sets
`RUSTIQ_PACKAGING=true` so both generators omit themselves from manuals and
completions. The installed executable retains `online` but disables the Cargo
features `manpages` and `completions`, so it does not offer generation commands
or link their dependencies. Other packagers can set the same variable when
generating assets for a binary built without those features.
Nix checks build and test all features separately; package installation checks
verify the installed assets and the absence of generator commands.

## Useful references

- [Nix flakes](https://nix.dev/concepts/flakes.html) and [`nix develop`](https://nix.dev/manual/nix/2.28/command-ref/new-cli/nix3-develop.html).
- [Rustup toolchain file](https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file) and [Cargo commands](https://doc.rust-lang.org/cargo/commands/index.html).
- [PySCF documentation](https://pyscf.org/user.html), [uv documentation](https://docs.astral.sh/uv/), and [JupyterLab documentation](https://jupyterlab.readthedocs.io/).
