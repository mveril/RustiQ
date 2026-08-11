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

          rustiq = rustPlatform.buildRustPackage {
            pname = "RustiQ";
            version = "0.1.0-alpha.1";
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
        in
        {
          packages.default = rustiq;

          devShells.default = pkgs.mkShell {
            strictDeps = true;

            packages =
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
                llvmPackages.bintools
                perf
              ]
              ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
                libiconv
                samply
              ];

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

          formatter = pkgs.nixfmt;

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

          checks.tests = rustiq;
        };
    };
}
