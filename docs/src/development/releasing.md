# Releasing RustiQ

RustiQ uses [dist](https://axodotdev.github.io/cargo-dist/) to build and
publish binary archives to GitHub Releases. crates.io publication runs in a
separate workflow, so a registry outage does not block a binary release.
Neither workflow creates a release from a GitHub Release event; both are
triggered directly by a version tag, and crates.io publication can also be
started manually for an existing tag.

## Binary releases

The repository-root `dist-workspace.toml` pins dist 0.33.0, selects only the
`rustiq-chem` CLI package, and lists the six supported targets. The
`rustiq-chem-core` library remains a build dependency and is not packaged as an
application. The generated workflow is `.github/workflows/release.yml`; do not
edit it by hand.

Each release includes a `rustiq` executable (named `rustiq.exe` on Windows),
the README, and the MIT and Apache license files. Unix archives use `.tar.gz`
and Windows archives use `.zip`. Dist adds a `.sha256` file for each archive
and a `sha256.sum` manifest. Installers are disabled. Archives do not include
calculation samples or basis data.

To verify all release assets on Linux, download `sha256.sum` and all listed
assets into one directory, then run:

```sh
sha256sum --check sha256.sum
```

On macOS, use the built-in `shasum` command:

```sh
shasum -a 256 --check sha256.sum
```

On Windows PowerShell, compare the archive's entry in `sha256.sum` with:

```powershell
Get-FileHash .\rustiq-chem-x86_64-pc-windows-msvc.zip -Algorithm SHA256
```

The archive target identifies the operating system and CPU architecture; it
does not guarantee compatibility with every Linux distribution or older
system library. The Linux GNU archives require a compatible glibc runtime.

## Preparing a release

From the repository root, update the version in both package manifests and
update the CLI's `rustiq-chem-core` dependency requirement to that same
version:

- `Cargo.toml` (`rustiq-chem`)
- `crates/rustiq-core/Cargo.toml` (`rustiq-chem-core`)
- `Cargo.toml`'s `rustiq-core` dependency

After editing the manifests, regenerate `Cargo.lock` and commit it with the
version changes:

```sh
cargo check --workspace
git add Cargo.toml crates/rustiq-core/Cargo.toml Cargo.lock
git commit -m "Prepare release X.Y.Z"
```

The publishing workflow checks both package versions and the CLI dependency
requirement against the tag using Cargo metadata. After committing the version
change, check package contents and build the workspace:

```sh
cargo publish --dry-run --workspace --locked
cargo check --workspace --locked
```

Review the dist release plan and regenerate the workflow after changing the
dist configuration or updating dist:

```sh
dist plan
dist generate --mode ci
dist generate --mode ci --check
```

The Rust development shell includes the Nixpkgs `cargo-dist` package. The
generated workflow installs the exact dist version recorded in
`dist-workspace.toml`. Before regenerating locally, install that same version
if the Nixpkgs package differs:

```sh
cargo install cargo-dist --version 0.33.0 --locked
```

Commit and push the version changes, then create and push the matching tag:

```sh
git tag vX.Y.Z
git push origin HEAD
git push origin vX.Y.Z
```

For a prerelease, include its suffix in both package versions and in the tag,
for example `0.1.0-beta.1` and `v0.1.0-beta.1`. Dist recognizes the SemVer
prerelease suffix and marks the GitHub Release as a prerelease. Do not reuse an
existing tag or a crates.io package version.

The tag starts dist's generated workflow, which runs a release plan, builds
the configured platform archives, creates their checksums, and publishes the
assets to GitHub Releases. A pull request runs `dist plan` only; it does not
build release archives or publish a release.

## crates.io publication

`.github/workflows/publish-crates.yml` publishes both workspace crates when a
`v*` tag is pushed. Cargo publishes workspace members in dependency order, so
`rustiq-chem-core` is available before `rustiq-chem`. Before publishing, the
workflow uses the crates.io API to exclude package versions that already
exist. If a run publishes the core crate and then fails while publishing the
CLI, rerunning the same tag skips the existing core version and retries the
CLI. For a retry, open **Actions → Publish crates.io packages → Run
workflow**, enter the existing `v…` tag, and start the workflow.

The workflow currently authenticates with the Actions secret
`CARGO_REGISTRY_TOKEN`. Create it from a crates.io API token with only package
publishing permissions, then add it under the repository's Actions secrets
before the first release. Keep the token out of repository files and do not
grant permissions to yank crates.

Crates.io Trusted Publishing uses GitHub OIDC and avoids a long-lived token,
but crates.io requires each crate to have an initial token-authenticated
publication before a Trusted Publisher can be configured. For RustiQ's first
publication, use `CARGO_REGISTRY_TOKEN` to publish both packages through the
tag workflow or its manual execution for an existing tag. After both crates
exist, configure a Trusted Publisher for each crate in crates.io to trust this
repository and the publishing workflow. A follow-up change can then replace
the token authentication with OIDC; the binary release workflow does not use
or depend on the crates.io token.

Cargo may time out while waiting for a published version to appear in the
registry index even when the upload succeeded. Check the version on crates.io
before retrying; the workflow's existence check will skip a version already
published.
