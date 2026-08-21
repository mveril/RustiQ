{
  description = "RustiQ development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    crane.url = "github:ipetkov/crane";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-parts.url = "github:hercules-ci/flake-parts";

    pyproject-nix = {
      url = "github:pyproject-nix/pyproject.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    uv2nix = {
      url = "github:pyproject-nix/uv2nix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.pyproject-nix.follows = "pyproject-nix";
    };

    pyproject-build-systems = {
      url = "github:pyproject-nix/build-system-pkgs";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.pyproject-nix.follows = "pyproject-nix";
      inputs.uv2nix.follows = "uv2nix";
    };
  };

  outputs =
    inputs@{
      crane,
      flake-parts,
      nixpkgs,
      pyproject-build-systems,
      pyproject-nix,
      rust-overlay,
      uv2nix,
      ...
    }:
    let
      pythonWorkspace = uv2nix.lib.workspace.loadWorkspace { workspaceRoot = ./.; };

      pythonOverlay = pythonWorkspace.mkPyprojectOverlay {
        sourcePreference = "wheel";
      };
    in
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      # nixpkgs unstable 26.11 no longer supports x86_64-darwin.

      perSystem =
        { system, ... }:
        let
          pkgs = import nixpkgs {
            inherit system;

            overlays = [
              rust-overlay.overlays.default
            ];
          };

          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

          craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

          cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

          sourceRoot = ./.;

          cargoSource = pkgs.lib.cleanSourceWith {
            src = sourceRoot;
            filter =
              path: type:
              let
                relativePath = pkgs.lib.removePrefix "${toString sourceRoot}/" (toString path);
                inProjectTree =
                  directory: relativePath == directory || pkgs.lib.hasPrefix "${directory}/" relativePath;
              in
              craneLib.filterCargoSources path type
              || inProjectTree "assets"
              || inProjectTree "samples"
              || inProjectTree "tests/data";
          };

          commonCargoArgs = {
            pname = "RustiQ";
            version = cargoToml.package.version;
            src = cargoSource;
            strictDeps = true;
            cargoExtraArgs = "--locked --all-features";
          };

          cargoArtifacts = craneLib.buildDepsOnly commonCargoArgs;

          rustiq = craneLib.buildPackage (
            commonCargoArgs
            // {
              inherit cargoArtifacts;
            }
          );

          pythonBase = pkgs.callPackage pyproject-nix.build.packages {
            python = pkgs.python314;
          };

          pythonSet = pythonBase.overrideScope (
            pkgs.lib.composeManyExtensions [
              pyproject-build-systems.overlays.wheel
              pythonOverlay
            ]
          );

          pythonPyscf = pythonSet.mkVirtualEnv "rustiq-pyscf-environment" pythonWorkspace.deps.default;

          pythonScientific = pythonSet.mkVirtualEnv "rustiq-scientific-environment" pythonWorkspace.deps.all;

          rustPackages = [
            rustToolchain
            pkgs.rust-analyzer
          ];

          developmentPackages = with pkgs; [
            cargo-nextest
            cargo-deny
            cargo-llvm-cov
            cargo-criterion
            cargo-expand
            cargo-edit

            bacon
            cargo-watch
            git
            hyperfine
            just
            jq
            nixd
            nixfmt
            ripgrep
            ruff
            time
            cmake
            pkg-config
            uv
          ];

          platformPackages =
            pkgs.lib.optionals pkgs.stdenv.isLinux (
              with pkgs;
              [
                clang
                cargo-flamegraph
                gdb
                inferno
                perf
              ]
            )
            ++ pkgs.lib.optionals pkgs.stdenv.isDarwin (
              with pkgs;
              [
                libiconv
                samply
              ]
            );

          devPackages = rustPackages ++ developmentPackages ++ [ pythonScientific ] ++ platformPackages;

          pyscfCheck = pkgs.writeShellApplication {
            name = "pyscf-check";
            runtimeInputs = [
              rustiq
              pythonPyscf
            ];
            text = ''
              reference_tool="$PWD/tools/reference/compare_pyscf.py"
              if [[ ! -f "$reference_tool" ]]; then
                echo "pyscf-check must be run from the root of a RustiQ checkout." >&2
                exit 2
              fi

              RUSTIQ_BIN="${rustiq}/bin/RustiQ" exec python "$reference_tool" "$@"
            '';
          };
        in
        {
          packages = {
            default = rustiq;
            cargo-artifacts = cargoArtifacts;
            pyscf-environment = pythonPyscf;
          };

          apps = {
            pyscf-check = {
              type = "app";
              program = "${pyscfCheck}/bin/pyscf-check";
              meta.description = "Compare RustiQ sample energies against PySCF";
            };
          };

          devShells.default = pkgs.mkShell {
            strictDeps = true;
            packages = devPackages;

            env = {
              RUST_BACKTRACE = "1";
              UV_NO_SYNC = "1";
              UV_PYTHON = pythonSet.python.interpreter;
              UV_PYTHON_DOWNLOADS = "never";
              # aws-lc-sys compiles feature probes with -O0 -Werror in debug
              # builds, which conflicts with the Nix clang wrapper's fortify define.
              AWS_LC_SYS_CFLAGS = "-U_FORTIFY_SOURCE";
              # Keep symbols in release-like profiling builds. This matches
              # the [profile.profiling] section in Cargo.toml.
              CARGO_PROFILE_PROFILING_DEBUG = "true";
              # Allow local Cargo builds to link artifacts from target/ outside the Nix store.
              NIX_ENFORCE_PURITY = 0;
            };
          };

          formatter = pkgs.nixfmt-tree;

          checks.formatting = craneLib.cargoFmt { src = cargoSource; };

          checks.nix-formatting = pkgs.runCommand "rustiq-nix-formatting" { } ''
            ${pkgs.nixfmt}/bin/nixfmt --check ${./flake.nix}
            touch "$out"
          '';

          checks.clippy = craneLib.cargoClippy (
            commonCargoArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );

          checks.unit-tests = rustiq;
        };
    };
}
