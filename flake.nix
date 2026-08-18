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
  };

  outputs =
    inputs@{
      crane,
      flake-parts,
      nixpkgs,
      rust-overlay,
      ...
    }:
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

          pyscfSupported = builtins.elem system pkgs.python314Packages.pyscf.meta.platforms;

          pythonScientific = pkgs.python314.withPackages (
            pythonPackages:
            with pythonPackages;
            [
              numpy
              scipy
              matplotlib
              jupyterlab
              ipykernel
            ]
            ++ pkgs.lib.optional pyscfSupported pyscf
          );

          pythonPyscf = pkgs.python314.withPackages (pythonPackages: [ pythonPackages.pyscf ]);

          devPackages =
            with pkgs;
            [
              rustToolchain

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
              ripgrep
              rust-analyzer
              time
              cmake
              pkg-config
              pythonScientific
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
              clang
              cargo-flamegraph
              gdb
              inferno
              perf
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              libiconv
              samply
            ];

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

            # Stable executable environment for editors and other processes
            # that are not launched from an interactive Nix shell.
            dev-environment = pkgs.buildEnv {
              name = "rustiq-dev-environment";
              paths = devPackages;
              pathsToLink = [
                "/bin"
                "/share"
              ];
            };
          };

          apps = pkgs.lib.optionalAttrs pyscfSupported {
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
              # Keep symbols in release-like profiling builds. This matches
              # the [profile.profiling] section in Cargo.toml.
              CARGO_PROFILE_PROFILING_DEBUG = "true";
            };

            shellHook = ''
              echo "RustiQ development environment"
              echo "System: ${system}"
              echo "Rust: $(rustc --version)"
              echo "Cargo: $(cargo --version)"
              ${pkgs.lib.optionalString pyscfSupported ''
                echo "PySCF: $(python -c 'import pyscf; print(pyscf.__version__)')"
              ''}
              ${pkgs.lib.optionalString (!pyscfSupported) ''
                echo "PySCF: unavailable from nixpkgs on ${system}"
              ''}
            '';
          };

          formatter = pkgs.nixfmt-tree;

          checks.formatting = craneLib.cargoFmt { src = cargoSource; };

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
