# Releasing RustiQ

## What a version release publishes

A version tag (`vX.Y.Z`, including a prerelease suffix such as
`v0.1.0-alpha.1`) starts the GitHub Actions workflow in
`.github/workflows/release.yml`. The workflow builds the `rustiq` executable
for x86-64 Linux, ARM64 Linux, Intel and Apple Silicon macOS, and x86-64 and
ARM64 Windows. It attaches one archive per target to a GitHub Release. Each
archive contains the executable, the two license files, and `README.md`; it
does not bundle calculation samples or basis data.

The release job creates a single `SHA256SUMS` file covering all attached
archives. Download `SHA256SUMS` and all six archives into one directory to
check the full manifest. On Linux, verify it with:

```sh
sha256sum --check SHA256SUMS
```

On macOS, use the built-in `shasum`:

```sh
shasum -a 256 --check SHA256SUMS
```

If you downloaded only one platform archive, calculate its hash with
`sha256sum <archive>` on Linux or `shasum -a 256 <archive>` on macOS and compare
it with the matching filename and digest in `SHA256SUMS`. On Windows
PowerShell, compare the value from
`Get-FileHash .\RustiQ-vX.Y.Z-x86_64-pc-windows-msvc.zip -Algorithm SHA256`
with that entry.

The archives are built on native GitHub-hosted runners and include their target
triple in the filename. That identifies the build target; it is not a promise
that every Linux distribution or older system library version is compatible.
Choose a binary for the host operating system and architecture. Windows
executables are named `rustiq.exe`; other platforms use `rustiq`. Put the
executable in a directory on `PATH` to run `rustiq --help` from any working
directory. Calculation examples and local basis data remain separate; see the
[installation guide](../getting-started/installation.md) and
[basis-set guide](../user-guide/basis-sets.md).
The default build enables online basis lookup. Calculations still need their
requested basis available locally or online.

DEB, RPM, WinGet, HPC package-manager integration, and Nixpkgs packaging are
not part of this release workflow yet.

## Publishing to crates.io

The tag workflow publishes all workspace packages to crates.io with
`cargo publish --workspace --locked`. Cargo publishes the workspace members
and polls for each package to appear in the registry index. Cargo can time out
while polling even though an upload succeeded; check the package version on
crates.io before retrying a failed publication run. The GitHub Release is
created only after both the binary builds and Cargo publication succeed.

Before enabling releases, add a crates.io API token as the repository Actions
secret `CARGO_REGISTRY_TOKEN`. Keep the token scoped to publishing these
packages. Both packages must have the same version, and the CLI's dependency
on the core package must specify that version. The tag check rejects tags that
do not match both package versions. From a clean repository root, check package
contents before the first release with:

```sh
cargo publish --dry-run --workspace --locked
```

After publication, users can install the CLI from crates.io with:

```sh
cargo install --locked rustiq-chem --version 0.1.0-alpha.1
```

For later stable releases, omit `--version` to install the latest stable
version, or specify the desired version explicitly.

The installed command is `rustiq` (or `rustiq.exe` on Windows). Cargo's binary
directory must be on `PATH`. The default build enables online basis lookup;
calculations still need their requested basis available locally or online.
Cargo installation builds from source and requires the platform's native build
tools described in [manual setup](manual-setup.md#install-and-build). The
prebuilt GitHub archives do not require Rust or Cargo.

Replace `X.Y.Z` with the new version in the core and CLI package manifests,
including the CLI's `rustiq-chem-core` dependency requirement. Commit those
changes, then push the matching tag to trigger Cargo and binary publication.
The workflow checks that the tag, both package versions, and the CLI's core
dependency version match:

```sh
git tag vX.Y.Z
git push origin vX.Y.Z
```

For a prerelease, use the same prerelease version in both manifests and the tag,
for example `0.1.0-alpha.1` and `v0.1.0-alpha.1`. Do not reuse a published
crates.io version or an existing Git tag.
