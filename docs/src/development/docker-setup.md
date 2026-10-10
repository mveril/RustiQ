# Docker and VS Code Dev Container

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

Choose this setup when you want a prepared Linux workspace with VS Code
integration. Compare it with [native Nix](nix-setup.md) and
[manual installation](manual-setup.md) before choosing a calculation environment.

## Advantages and limitations for scientific computing

- **Advantages:** the repository supplies a consistent Linux workspace, pinned
  tools through Nix, and editor integration. Development dependencies stay in
  the container; the host needs Docker, Git, and VS Code.
- **Platform:** Rust, Cargo, and RustiQ run under Linux regardless of the host
  operating system. The resulting executable targets the container's Linux
  architecture; it is not a native Windows or macOS executable.
- **Virtualization:** Docker Engine directly on Linux shares the host kernel
  and does not require a virtual machine. Docker Desktop runs Linux containers
  in a Linux VM, including on Linux hosts. On Windows, its Linux backend uses
  WSL2 or Hyper-V. See the official [Docker Desktop VM documentation](https://docs.docker.com/desktop/troubleshoot-and-support/faqs/general/).
- **Resources and files:** the VM's CPU and memory allocation can restrict a
  calculation independently of the physical machine's capacity. Host file
  sharing can also affect I/O. Review [Desktop resource settings](https://docs.docker.com/desktop/settings-and-maintenance/settings/)
  and any [container resource limits](https://docs.docker.com/engine/containers/resource_constraints/)
  before large calculations. Images and the persistent Nix store consume disk
  space, and initial setup needs downloads.
- **HPC use:** this is a development workspace. Cluster policy may prohibit
  Docker or require another runtime; the supplied Dev Container does not
  configure a batch scheduler. Check the site's supported environment before
  choosing it for compute nodes.

There is no measured universal performance penalty documented for this setup.
For performance comparisons, use release builds and record the actual CPU,
available memory, architecture, filesystem, and any VM or container limits.
Matching the container architecture to the processor avoids introducing
cross-architecture emulation into the comparison.

### Prepare your computer

1. Install [Visual Studio Code](https://code.visualstudio.com/download).
2. Install and start [Docker Desktop](https://docs.docker.com/desktop/), or
   [Docker Engine](https://docs.docker.com/engine/install/) on Linux.
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

