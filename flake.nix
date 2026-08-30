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

    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

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
      treefmt-nix,
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

          treefmt = treefmt-nix.lib.evalModule pkgs {
            projectRootFile = "flake.nix";
            programs = {
              nixfmt.enable = true;
              prettier.enable = true;
              ruff-format.enable = true;
              rustfmt = {
                enable = true;
                edition = "2021";
                package = rustToolchain;
              };
              taplo.enable = true;
            };
          };

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

          pythonPyscfTest = pythonSet.mkVirtualEnv "rustiq-pyscf-test-environment" (
            pkgs.lib.mapAttrs (_: _: [ "test" ]) pythonWorkspace.deps.default
          );

          pythonScientific = pythonSet.mkVirtualEnv "rustiq-scientific-environment" pythonWorkspace.deps.all;

          rustBuildPackages = with pkgs; [
            rustToolchain
            cmake
            pkg-config
          ];

          rustDevelopmentPackages = with pkgs; [
            rust-analyzer
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
            time
          ];

          rustBuildPlatformPackages = pkgs.lib.optionals pkgs.stdenv.isDarwin [ pkgs.libiconv ];

          rustDevelopmentPlatformPackages =
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
                samply
              ]
            );

          minimalRustPackages = rustBuildPackages ++ rustBuildPlatformPackages;

          completeRustPackages =
            minimalRustPackages ++ rustDevelopmentPackages ++ rustDevelopmentPlatformPackages;

          pythonDevelopmentPackages = with pkgs; [
            ruff
            uv
          ];

          commonRustEnv = {
            RUST_BACKTRACE = "1";
            # aws-lc-sys compiles feature probes with -O0 -Werror in debug
            # builds, which conflicts with the Nix clang wrapper's fortify define.
            AWS_LC_SYS_CFLAGS = "-U_FORTIFY_SOURCE";
            # Keep symbols in release-like profiling builds. This matches
            # the [profile.profiling] section in Cargo.toml.
            CARGO_PROFILE_PROFILING_DEBUG = "true";
            # Allow local Cargo builds to link artifacts from target/ outside the Nix store.
            NIX_ENFORCE_PURITY = 0;
          };

          pythonEnv = {
            UV_NO_SYNC = "1";
            UV_PYTHON = pythonSet.python.interpreter;
            UV_PYTHON_DOWNLOADS = "never";
          };

          mkDevShell =
            {
              packages,
              extraEnv ? { },
            }:
            pkgs.mkShell {
              strictDeps = true;
              inherit packages;
              env = commonRustEnv // extraEnv;
            };

          pyscfCheck = pkgs.writeShellApplication {
            name = "pyscf-check";
            runtimeInputs = [
              rustiq
              pythonPyscf
            ];
            text = ''
              reference_tests="$PWD/tools/reference"
              if [[ ! -d "$reference_tests" ]]; then
                echo "pyscf-check must be run from the root of a RustiQ checkout." >&2
                exit 2
              fi

              RUSTIQ_BIN="${rustiq}/bin/RustiQ" exec pytest "$reference_tests" "$@"
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
              meta.description = "Run RustiQ/PySCF reference tests with pytest";
            };
          };

          devShells = rec {
            "mini-rust" = mkDevShell { packages = minimalRustPackages; };

            rust = mkDevShell { packages = completeRustPackages; };

            "mini-pyscf" = mkDevShell {
              packages = minimalRustPackages ++ [ pythonPyscfTest ];
              extraEnv = pythonEnv;
            };

            full = mkDevShell {
              packages = completeRustPackages ++ pythonDevelopmentPackages ++ [ pythonScientific ];
              extraEnv = pythonEnv;
            };

            default = full;
          };

          formatter = treefmt.config.build.wrapper;

          checks.formatting = treefmt.config.build.check sourceRoot;

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
