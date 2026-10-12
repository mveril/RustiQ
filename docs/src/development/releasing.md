# Releasing RustiQ

RustiQ uses [dist](https://axodotdev.github.io/cargo-dist/) to build and
publish binary archives to GitHub Releases. crates.io publication runs in a
separate workflow, so a registry outage does not block a binary release.
Neither workflow creates a release from a GitHub Release event; both are
triggered directly by a version tag, and crates.io publication can also be
started manually for an existing tag.

## Binary releases

The repository-root `dist-workspace.toml` pins dist 0.33.0, selects only the
`rustiq-chem` CLI package, and lists the six supported targets. Pull requests
run `dist plan`; this checks the release plan without building or publishing
artifacts. The generated workflow also runs on non-main pull requests because
dist 0.33.0 does not expose a pull-request base-branch filter. A pull request
cannot enter the publishing jobs.

All six target builds use native GitHub-hosted runners: Linux x64 on
`ubuntu-22.04`, Linux ARM64 on `ubuntu-24.04-arm`, macOS Intel on
`macos-15-intel`, Apple Silicon on `macos-14`, Windows x64 on `windows-2025`,
and Windows ARM64 on `windows-11-arm`. The configuration explicitly selects
the ARM runners and preserves the native Windows runners.

The
`rustiq-chem-core` library remains a build dependency and is not packaged as an
application. The generated workflow is `.github/workflows/release.yml`; do not
edit it by hand.

### Release rehearsal

Run **Actions → Release artifact smoke test → Run workflow** and select the
commit, branch, or tag to rehearse. The manually triggered workflow builds the
configured target archive with the pinned dist version on each target's native
GitHub-hosted runner. It extracts that archive, verifies its dist-generated
SHA-256 checksum, starts the packaged `rustiq` executable, checks `--help`,
and runs the checked-in H2/STO-3G regression with a copied local basis fixture
and JSON output. Per-target results appear in the job summary; archives, dist
manifests, and command logs are uploaded as workflow artifacts.

This checks archive integrity and basic executable operation for the six
configured targets. The H2 energy comparison reuses the regression reference
and is a packaging smoke test, not broad scientific validation, independent
platform validation, or proof of compatibility with every supported operating
system. The runs use GitHub-hosted runners, which are not pristine operating
system installations. A `sha256.sum` release manifest may only be created in
the tagged release's combined build; when unavailable, the rehearsal verifies
the per-archive checksum emitted by dist and fails if neither checksum is
available. A checksum made during a rehearsal does not independently validate
a published release.

The rehearsal has read-only repository permissions and only a
`workflow_dispatch` trigger. It does not run dist's `host` command, upload
release assets, create a GitHub Release, create tags, or publish crates. It
complements `.github/workflows/release.yml`, whose tagged-release path builds
and publishes the release assets; ordinary pull requests continue to use the
generated plan-only check.

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
dist configuration or updating dist. The pinned generator checks that the
generated release workflow is synchronized with `dist-workspace.toml`:

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

Release tags must match `vX.Y.Z` or `vX.Y.Z-prerelease`, such as `v1.2.3` or
`v0.1.0-beta.1`. For a prerelease, include the same suffix in both package
versions and the tag. Dist recognizes the suffix and marks the GitHub Release
as a prerelease. Do not reuse an existing tag or a crates.io package version.

The tag starts dist's generated workflow, which builds the configured
platform archives, creates their checksums, and publishes the assets to
GitHub Releases. A dist `plan-jobs` reusable workflow validates the tag format
and requires its commit to be an ancestor of `main` before artifact builds can
proceed. This accepts prerelease suffixes such as `-alpha.1`. A failed
validation blocks the build and therefore the publishing jobs.

## crates.io publication

`.github/workflows/publish-crates.yml` publishes both workspace crates when a
`v*` tag on `main` is pushed. Cargo publishes workspace members in dependency
order, so
`rustiq-chem-core` is available before `rustiq-chem`. Before publishing, the
workflow uses the crates.io API to exclude package versions that already
exist. If a run publishes the core crate and then fails while publishing the
CLI, rerunning the same tag skips the existing core version and retries the
CLI. For a retry, open **Actions → Publish crates.io packages → Run
workflow**, enter the existing `v…` tag, and start the workflow.
This workflow remains separate from the dist binary release workflow and can
also be run manually for an existing tag.

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
