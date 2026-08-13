{
  description = "RustiQ development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs =
    inputs@{
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

          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };

          cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

          rustiq = rustPlatform.buildRustPackage {
            pname = "RustiQ";
            version = cargoToml.package.version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            doCheck = true;
          };

          pythonScientific = pkgs.python314.withPackages (
            pythonPackages: with pythonPackages; [
              pyscf
              numpy
              scipy
              matplotlib
              jupyterlab
              ipykernel
            ]
          );

          pyscfSupported = builtins.elem system pkgs.python314Packages.pyscf.meta.platforms;

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
            ]
            ++ pkgs.lib.optionals pyscfSupported [
              # PySCF is currently available from nixpkgs on x86_64-linux
              # and aarch64-darwin.
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
        in
        {
          packages = {
            default = rustiq;

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

          checks.formatting =
            pkgs.runCommand "rustiq-formatting"
              {
                nativeBuildInputs = [ rustToolchain ];
                src = ./.;
              }
              ''
                cargo fmt --manifest-path "$src/Cargo.toml" --all --check

                touch "$out"
              '';

          checks.clippy = rustPlatform.buildRustPackage {
            pname = "rustiq-clippy";
            version = cargoToml.package.version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            buildPhase = ''
              runHook preBuild
              cargo clippy --all-targets --all-features -- -D warnings
              runHook postBuild
            '';

            installPhase = ''
              runHook preInstall
              touch "$out"
              runHook postInstall
            '';

            doCheck = false;
          };

          checks.build = rustiq;
        };
    };
}
