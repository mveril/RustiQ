# Development environments

This chapter assumes you are familiar with running scientific calculations,
but have not used the software tools below. The recommended setup gives you a
prepared workspace for editing calculation inputs, compiling RustiQ, and
comparing results with PySCF.

## What the tools do

| Tool | Role in your workflow |
| --- | --- |
| [VS Code](https://code.visualstudio.com/) | The application in which you open the project, edit input files, and run commands in a terminal |
| [Docker](https://docs.docker.com/get-started/docker-overview/) | Runs a separate Linux working environment on your computer, with its own installed tools |
| [Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers) | Connects VS Code to that Linux environment and prepares it from the configuration supplied with RustiQ |
| [Nix](https://nixos.org/learn/) | Installs the versions of Rust and Python tools selected by the project; it is already included in the Dev Container |
| [WSL2](https://learn.microsoft.com/en-us/windows/wsl/about) | Runs Linux within Windows; it is another way to use the Linux setup and is also used by Docker Desktop's Windows WSL2 backend |
| [Git](https://git-scm.com/) | Downloads a working copy of the project and records changes to its files |

A **container** is the separate working environment run by Docker. The
**host** is your own computer. A **terminal** is a window in which you type
commands, much like the command-line sessions used to run quantum chemistry
programs. A **shell** interprets those commands. A Nix development shell also
makes the project's selected tools available in that session.

The next section walks through the recommended setup. Nix and WSL alternatives
are explained later for readers who want to manage their environment directly.

## Recommended: VS Code Dev Container

[Visual Studio Code](https://code.visualstudio.com/) (usually called VS Code)
is a free application for editing and managing software projects. You can think
of it as a text editor with a project file browser, a built-in terminal, and
optional extensions that add features such as Rust error highlighting,
debugging, or Nickel support. For this project, it gives you one place to edit
files, run commands, and see errors.

A **Dev Container** is a prepared Linux environment for the project. VS Code
shows the files and editor on your computer, while Rust, Nix, and the other
development tools run inside the container. This avoids installing and
configuring those tools individually on your computer. The repository includes
the container configuration, so contributors can use the same setup.

This is the recommended starting point if you want the tools ready to use
without assembling them yourself. You can also work from a terminal with Cargo
or Nix and use another editor; see [Nix without direnv](#nix-without-direnv).

### Prepare your computer

1. Install [Visual Studio Code](https://code.visualstudio.com/download).
2. Install and start [Docker Desktop](https://docs.docker.com/desktop/).
   Follow its instructions for your operating system. On Windows, its WSL2
   backend requires WSL2; the [official setup guide](https://docs.docker.com/desktop/setup/install/windows-install/)
   explains that requirement. Docker must be configured to run Linux containers.
3. In VS Code, open the **Extensions** view from the left sidebar, search for
   **Dev Containers**, and install the extension published by Microsoft.
4. Install [Git](https://git-scm.com/downloads) to download the project.

### Open RustiQ in its prepared environment

Open a terminal on your computer and download the project:

```sh
git clone https://github.com/mveril/RustiQ.git
cd RustiQ
```

In VS Code, choose **File > Open Folder** and select the downloaded `RustiQ`
folder. Open **View > Command Palette**, search for
**Dev Containers: Reopen in Container**, and select it. The command palette
lets you find VS Code actions by name. The
first creation can take several minutes while Docker builds the image and Nix
downloads the toolchain pinned by `flake.lock`. Subsequent starts reuse the
Docker image and Nix store.

The container is based on Debian Bookworm and installs only Nix and direnv at
the system level. The FlakeEnv extension loads `devShells.default` directly and
propagates its environment to terminals, tasks, debuggers, and language
servers. Rust, Nix, TOML, Nickel, dependency, Python, Jupyter, and LLDB support
is installed as a small explicit extension list rather than through extension
packs with overlapping behavior.

The Rust toolchain, rust-analyzer, nixd, nixfmt, Nickel, its language server,
Ruff, scientific Python stack, and development utilities are provided by the
Nix shell. The Dev Container also installs these VS Code extensions:

- [FlakeEnv](https://marketplace.visualstudio.com/items?itemName=auricvex.flake-env) loads the flake environment into editor processes.
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) provides Rust language support; [CodeLLDB](https://marketplace.visualstudio.com/items?itemName=vadimcn.vscode-lldb) provides debugging.
- [Nix IDE](https://marketplace.visualstudio.com/items?itemName=jnoortheen.nix-ide) and [Nickel](https://marketplace.visualstudio.com/items?itemName=Tweag.vscode-nickel) provide Nix and Nickel language support.
- [Even Better TOML](https://marketplace.visualstudio.com/items?itemName=tamasfe.even-better-toml) supports TOML, while [Prettier](https://marketplace.visualstudio.com/items?itemName=esbenp.prettier-vscode) formats JSON, YAML, and Markdown.
- [Dependi](https://marketplace.visualstudio.com/items?itemName=fill-labs.dependi) helps inspect dependencies; [Python](https://marketplace.visualstudio.com/items?itemName=ms-python.python), [Jupyter](https://marketplace.visualstudio.com/items?itemName=ms-toolsai.jupyter), and [Ruff](https://marketplace.visualstudio.com/items?itemName=charliermarsh.ruff) support the Python reference tools.

The first activation can take several minutes; later starts reuse the persistent Nix store.
Because `.envrc` execution requires explicit trust, run `direnv allow` once in
the container if FlakeEnv reports that it is blocked, then run **FlakeEnv:
Reload Environment**.

When the workspace is ready, choose **Terminal > New Terminal** in VS Code.
Commands in this terminal run inside the prepared Linux environment. Verify
that the tools are available:

```sh
cargo --version
rustc --version
rust-analyzer --version
cargo test
```

After changing `.devcontainer/devcontainer.json`, rebuild the container. After
changing `flake.nix` or `flake.lock`, run **FlakeEnv: Reload Environment**; a
container rebuild is only needed when the Dev Container configuration changes.

### Dev Container Troubleshooting On Windows

- If Docker reports that `dockerDesktopLinuxEngine` cannot be found, start
  Docker Desktop and wait until `docker info` succeeds before reopening the
  repository.
- If container creation fails while mounting a path such as
  `\\wsl.localhost\<distribution>\mnt\wslg\runtime-dir\wayland-0`, disable
  **Dev Containers: Mount Wayland Socket** in the VS Code user settings. The
  equivalent JSON setting is `"dev.containers.mountWaylandSocket": false`.
  This only disables Linux GUI forwarding; it does not affect Docker, Rust,
  Nix, or terminal access.
- Nix inside the container is installed by the Dev Container feature. It is
  independent of any Nix or NixOS installation in WSL.
- If a flake change is not visible in the editor, run **FlakeEnv: Reload
  Environment** and restart the affected language server if necessary.

## Choosing A Development Environment

RustiQ can be developed from any editor or terminal using the repository's
Nix flake or a regular Rust installation. The VS Code Dev Container is an
optional editor integration. `direnv` is also optional: it automates entering
and leaving a Nix development shell.

See [Nix packages and shells](nix-packages.md) for the role of each tool in
`flake.nix` and the shells that include it.

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

On Windows, the Dev Container is the simplest way to use the complete pinned
environment. The alternatives are the native Cargo workflow below or the Linux
flake through WSL2. WSL2 can run either
[NixOS-WSL](https://nix-community.github.io/NixOS-WSL/) or another Linux
distribution with the Nix package manager installed. Native Windows itself is
not one of the systems currently declared by `flake.nix`.

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

### Without Nix Or direnv

Install Git and Rust through [`rustup`](https://rustup.rs/), then use Cargo
directly on Linux, macOS, or Windows:

```sh
git clone https://github.com/mveril/RustiQ.git
cd RustiQ
rustup show
cargo build
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
