# Development environments

Choose an environment according to the operating system on which you want
RustiQ to run, the tools you want prepared, and your compute site's policies.
Docker is an optional development convenience; it is not a universal
recommendation for scientific calculations.

| Setup | Where Rust and RustiQ run | Advantages | Limitations |
| --- | --- | --- | --- |
| [Docker / Dev Container](docker-setup.md) | Linux inside a container on every host | Prepared workspace and VS Code integration | Linux target only; Desktop uses a VM; resource and file-sharing settings need attention |
| [Nix](nix-setup.md) | Native Linux or Apple Silicon macOS; Linux in WSL2 on Windows | Pinned tools without requiring Docker | Limited flake platforms; store and installation requirements; WSL2 uses a VM |
| [Manual / Cargo](manual-setup.md) | Native Linux, macOS, or Windows | Direct host execution and control over installed tools | Native dependencies and optional scientific tools must be installed separately |

For native execution on Windows or Intel macOS, choose manual installation.
On Linux or Apple Silicon macOS, Nix prepares tools while retaining native
execution. Choose Docker when a shared Linux development workspace is useful
and its host integration suits your needs.

For HPC workloads, consider the compute node's operating system, available
memory and CPUs, filesystem, and scheduler policies. A native setup avoids
adding a VM on Windows or macOS; Docker Engine directly on Linux shares the
host kernel. These distinctions do not establish a performance ranking:
measure representative calculations with release builds on the intended
execution environment. The Docker page explains resource limits and platform
boundaries in more detail.

Each setup page contains its prerequisites, commands, advantages, and limits.
See [known supported platforms](platforms.md) for compatibility evidence and
[Nix packages and shells](nix-packages.md) for the pinned tool inventory.

## Terms used in the setup guides

A **host** is the machine running your environment. A **container** isolates
processes and their installed tools while using a Linux kernel. A **virtual
machine (VM)** runs a guest operating system with its own kernel. **WSL2** runs
Linux in a VM on Windows. A **terminal** lets you type commands; a **shell**
interprets them. A Nix development shell makes selected tools available in that
session. **Cargo** builds Rust programs, and **rustup** installs Rust toolchains.
