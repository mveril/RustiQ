# Nix setup

Nix supplies the project's pinned toolchain without requiring Docker or VS Code.
A development shell makes tools available to processes on the current operating
system; it is not a virtual machine. See [Nix packages and shells](nix-packages.md)
for the individual tools provided by the flake.

## Advantages and limitations

- **Advantages:** `flake.lock` pins the environment dependencies. Linux and
  Apple Silicon macOS users compile and run RustiQ natively with prepared Rust
  and optional PySCF tools. Any editor or terminal can be used.
- **Platforms:** this flake supports Linux x86_64 and AArch64 and Apple Silicon
  macOS. It does not declare native Windows or Intel macOS support.
- **Windows:** WSL2 adds a Linux VM and produces Linux executables. Nix inside
  WSL2 does not remove that virtualization layer.
- **Maintenance:** Nix installation, flakes, and the store introduce concepts
  to learn. Initial downloads or local dependency builds can take time; the
  store requires disk space. `direnv` is optional.
- **HPC use:** native Nix avoids a container runtime on supported hosts, but
  installation and access to `/nix/store` depend on cluster policy. Build on a
  suitable build node and follow the site's scheduler and resource rules.
  Copying a Nix executable alone does not carry its store dependencies to
  another machine.

If Nix is new to you, start with the official [introduction to
Nix](https://nixos.org/why-nix/) and [learning resources](https://nixos.org/learn/).
Nix is a package manager and development-environment tool; NixOS is a complete
Linux distribution built around it. You do not need to replace your operating
system with NixOS to use this repository's flake.

Useful references include the official [Nix flakes guide](https://nix.dev/concepts/flakes.html),
[`nix develop` reference](https://nix.dev/manual/nix/2.28/command-ref/new-cli/nix3-develop.html),
[NixOS-WSL documentation](https://nix-community.github.io/NixOS-WSL/),
[direnv installation guide](https://direnv.net/docs/installation.html), and
[nix-direnv](https://github.com/nix-community/nix-direnv). For tools used outside
the flake, see [Rustup](https://rustup.rs/) and the [uv documentation](https://docs.astral.sh/uv/).

The flake supports the following platforms:

- Linux on x86_64 and AArch64;
- macOS on Apple Silicon.

The pinned nixpkgs revision no longer supports Intel macOS (`x86_64-darwin`).
Use the native Cargo workflow on that platform.

On Windows, use the Linux flake through WSL2, with either
[NixOS-WSL](https://nix-community.github.io/NixOS-WSL/) or another Linux
distribution with Nix installed. This builds and runs Linux executables.
For a native Windows executable, use [manual installation](manual-setup.md).

### Getting Nix

Choose the case that matches your machine:

- **On NixOS, including NixOS-WSL:** Nix is already installed as part of the
  operating system. [NixOS-WSL installation
  instructions](https://nix-community.github.io/NixOS-WSL/install.html) are
  available for users who want to run NixOS directly under WSL2. Make sure the
  modern Nix command and flakes are enabled in your NixOS configuration:

  ```nix
  nix.settings.experimental-features = [ "nix-command" "flakes" ];
  ```

  Apply the configuration with `sudo nixos-rebuild switch`, then continue with
  `nix develop` or the direnv workflow below.

- **On another Linux distribution, macOS, or a non-NixOS WSL2 distribution:**
  install Nix separately as an additional package manager. It works alongside
  tools such as `apt`, `dnf`, `pacman`, or Homebrew and does not replace them.
  Follow the official [Nix download and installation
  instructions](https://nixos.org/download/) for your platform, restart the
  shell if requested, and verify the installation with `nix --version`. The
  official page recommends a multi-user installation when the platform supports
  it.

In either case, Nix reads `flake.nix` and `flake.lock` from this repository to
create the same project-specific toolchain without installing those development
tools globally. The first invocation may take some time because Nix must
download the pinned dependencies; later invocations reuse its local store.

### Nix Without direnv

Install Nix with flakes enabled, clone the repository, and enter the development
shell manually:

```sh
git clone https://github.com/mveril/RustiQ.git
cd RustiQ
nix develop
```

The default `full` shell provides the Rust toolchain selected by
`rust-toolchain.toml`, the development utilities, and a Python environment
built from the checked-in `uv.lock`. PySCF and the scientific Python
dependencies are available on every platform declared by the flake. Nix places
that environment directly on `PATH`, so no virtual environment needs to be
created or activated:

```sh
python -c "import pyscf; print(pyscf.__version__)"
```

Platform-specific profiling and debugging tools are included where available.
Run the usual Cargo commands inside the shell:

```sh
cargo build
cargo test
cargo run -- run samples/h2/sto-3g/calculation.toml
```

Four shells are available so that contributors only load the tools needed for
their current task:

| Shell        | Contents                                                                 | Command                    |
| ------------ | ------------------------------------------------------------------------ | -------------------------- |
| `mini-rust`  | Rust toolchain and native build dependencies                             | `nix develop .#mini-rust`  |
| `rust`       | Complete Rust development, debugging, and profiling tools without Python | `nix develop .#rust`       |
| `mini-pyscf` | Minimal Rust build environment plus Python and PySCF                     | `nix develop .#mini-pyscf` |
| `full`       | Complete Rust and scientific Python environment                          | `nix develop .#full`       |

Running `nix develop` without a shell name selects `full`.

Leave the environment with `exit` or Ctrl-D. You can also build the default Nix
package without entering the development shell:

```sh
nix build
```

### Nix With direnv

Install both Nix and `direnv`, enable the direnv hook for your shell, then run:

```sh
git clone https://github.com/mveril/RustiQ.git
cd RustiQ
direnv allow
```

The tracked `.envrc` loads the `full` shell automatically whenever you enter
the repository and unloads it when you leave. To select a lighter shell for one
checkout, create an ignored `.envrc.local`, then allow the updated environment:

```sh
printf '%s\n' 'export RUSTIQ_DEV_SHELL=rust' > .envrc.local
direnv allow
```

Valid values are `mini-rust`, `rust`, `mini-pyscf`, and `full`. The tracked
`.envrc` rejects other values before passing the selection to Nix. `direnv
allow` is deliberately required the first time, and again after either envrc
file changes, so that shell code is not executed without review. Use `direnv
deny` to revoke permission.

Inside the Dev Container, FlakeEnv performs this integration for VS Code. The
tracked `.envrc` remains useful for developers who use direnv in another editor
or terminal.

If `use flake` is unknown, install or configure
[`nix-direnv`](https://github.com/nix-community/nix-direnv), or use `nix develop`
directly. Some direnv/Nix installations already provide this integration.

