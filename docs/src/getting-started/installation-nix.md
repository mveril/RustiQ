# Installing with Nix

Nix is one of the three release installation channels, alongside a
[GitHub Releases archive](installation.md#install-a-prebuilt-release-binary)
and the [Cargo package](installation.md#install-with-cargo). This page covers
the Nix-specific choices.

Install [Nix](https://nixos.org/download/) and enable the `nix-command` and
`flakes` experimental features. The flake supports x86-64 Linux, ARM64 Linux,
and Apple Silicon macOS. On Windows, use Nix inside WSL2 to run the Linux
package. See [Nix setup](../development/nix-setup.md#getting-nix) for enabling
flakes and platform prerequisites. The first install needs network access and
may compile dependencies. Install the standard package from any directory with:

```sh
nix profile install github:mveril/RustiQ
rustiq --version
```

The unqualified `github:mveril/RustiQ` reference follows GitHub's default
branch, `main`. That branch contains released versions only. To keep a fixed
version, use a release tag (for example,
`github:mveril/RustiQ/v0.1.0-beta.1`) or a commit revision instead.
An unqualified reference is captured only when a consuming configuration's
`flake.lock` is updated; a tag makes the selected release explicit.

## Home Manager without a configuration flake

If your Home Manager configuration uses `home.nix` directly, create a
`rustiq.nix` file beside it:

```nix
{ pkgs, ... }:
let
  rustiq = builtins.getFlake "github:mveril/RustiQ";
in
{
  home.packages = [
    rustiq.packages.${pkgs.stdenv.hostPlatform.system}.default
  ];
}
```

Add the module to the imports in `home.nix`, keeping any existing imports:

```nix
imports = [ ./rustiq.nix ];
```

Apply the configuration using your usual command. For Home Manager integrated
as a NixOS module, run `sudo nixos-rebuild switch --impure`; for standalone Home
Manager, run `home-manager switch --impure`. The unlocked GitHub reference
requires impure evaluation. Check `rustiq --version` afterward. The reference tracks
the release-only default branch, `main`, and is not pinned by a configuration
`flake.lock`; use a revision in the URL
for a fixed version. The standard package is portable across machines of the
same supported platform.

## Home Manager and NixOS

The following snippets extend an existing **flake-based** configuration. Add
RustiQ as an input to your configuration's `flake.nix`:

```nix
inputs.rustiq.url = "github:mveril/RustiQ";
```

The configuration's `flake.lock` pins that input. To update it later, run
`nix flake update rustiq` in the configuration directory, then apply the
configuration again. Keep your existing inputs, modules, and state version.
If the configuration is a Git flake, add any newly created module files to Git
before applying it so that Nix can see them.

### Home Manager

In the existing `outputs` function, bind the inputs as `inputs@{ ... }`, and
pass them to your existing `home-manager.lib.homeManagerConfiguration`:

```nix
extraSpecialArgs = { inherit inputs; };
```

Merge this with any existing `extraSpecialArgs`. In your `home.nix`, add
`inputs` to the module arguments and add RustiQ to `home.packages`:

```nix
{ pkgs, inputs, ... }:
{
  home.packages = [
    inputs.rustiq.packages.${pkgs.stdenv.hostPlatform.system}.default
  ];
}
```

Merge the list with your existing packages. From your configuration directory,
apply it with `home-manager switch --flake .#YOUR_USER` and check
`rustiq --version`. Replace `YOUR_USER` with your Home Manager configuration
name. See the [Home Manager flake guide](https://nix-community.github.io/home-manager/nix-flakes/standalone.html).
For Home Manager as a NixOS module, pass the inputs through
`home-manager.extraSpecialArgs` instead.

### NixOS configuration.nix

In the existing `outputs` function, bind the inputs as `inputs@{ ... }`, and
pass them to your existing `nixpkgs.lib.nixosSystem`:

```nix
specialArgs = { inherit inputs; };
```

Merge this with any existing `specialArgs`. Create `rustiq.nix` beside your
`configuration.nix`. This complete NixOS module installs the standard portable
RustiQ package:

```nix
{{#include rustiq.nix}}
```

Then import it in the existing `configuration.nix`, preserving any other imports:

```nix
{ ... }:
{
  imports = [ ./rustiq.nix ];
}
```

Alternatively, add `./rustiq.nix` to the `modules` list in `nixosSystem`.
From the configuration directory, apply it with
`sudo nixos-rebuild switch --flake .#YOUR_HOST` and check `rustiq --version`.
Replace `YOUR_HOST` with your NixOS configuration name. See the
[NixOS package management guide](https://nixos.org/manual/nixos/stable/#sec-declarative-package-management).

## Optional CPU-specific build

The examples above use the portable standard package. For NixOS or Home
Manager, replace that package expression with this one only when a build tied
to the build machine's CPU is appropriate:

```nix
(inputs.rustiq.legacyPackages.${pkgs.stdenv.hostPlatform.system}.cpuTarget "native")
```

For a profile installation from any directory, use one command instead:

```sh
nix profile install --impure --expr '(builtins.getFlake "github:mveril/RustiQ").legacyPackages.${builtins.currentSystem}.cpuTarget "native"'
```

List compiler CPU names with `rustc --print target-cpus` (for example inside
`nix develop github:mveril/RustiQ#rust`). `native` selects the **build machine's CPU**, which may differ from the machine
running RustiQ when using remote builders. Its detected CPU is not recorded in
the Nix build identity: cached results may have been built for a different CPU.
For reproducible builds or deployment across machines, replace `native` with a
compatible explicit CPU name such as `znver3`. A binary may fail on machines
with fewer processor features. This setting applies to Rust-compiled code and
does not promise optimization of every native dependency. The `--impure` flag
allows the command-line expression to obtain the current system and resolve the
unlocked GitHub reference.

## Build from a checkout

From the root of a RustiQ checkout, build the default package and run it
without adding it to your profile:

```sh
nix build
./result/bin/rustiq --version
```

For development, clone `develop` explicitly because the default branch is
release-only:

```sh
git clone --branch develop --single-branch https://github.com/mveril/RustiQ.git
cd RustiQ
nix develop
```

The package includes the offline manual and shell completions. Run `rustiq docs`
after installing, or `./result/bin/rustiq docs` from the checkout. See the
[manual instructions](installation.md#consult-the-manual) and the
[Nix package guide](../development/nix-packages.md#installed-manual-and-completions).

The package enables online basis downloads but does not bundle sample inputs or basis sets.
Download or import a basis before running a calculation; see
[obtaining a basis after installation](installation.md#after-installation-obtain-a-basis-and-run-a-calculation) and the
[first calculation](first-calculation.md). For development shells, see
[Nix setup](../development/nix-setup.md).
