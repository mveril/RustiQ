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
        "x86_64-darwin"
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

          rustToolchain =
            pkgs.rust-bin.fromRustupToolchainFile
              ./rust-toolchain.toml;

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

          # PySCF currently has the best fit with the x86_64 Linux system used
          # by this project. Keep the shell portable for the other systems
          # advertised above, while making the native environment reproducible.
          pythonPyscf = pkgs.python314.withPackages (pythonPackages: with pythonPackages; [
            pyscf
            numpy
            scipy
            matplotlib
            jupyterlab
            ipykernel
          ]);
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
              ++ pkgs.lib.optionals (system == "x86_64-linux") [
                # This provides the regular `python` command with PySCF
                # available, matching the system environment.
                pythonPyscf
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
              ${pkgs.lib.optionalString (system == "x86_64-linux") ''
                echo "PySCF: $(python -c 'import pyscf; print(pyscf.__version__)')"
              ''}
            '';
          };

          formatter = pkgs.nixfmt-rfc-style;

          checks.formatting = pkgs.runCommand "rustiq-formatting" {
            nativeBuildInputs = [ rustToolchain ];
            src = ./.;
          } ''
            cp -r "$src" source
            chmod -R u+w source
            cd source

            cargo fmt --all --check

            touch "$out"
          '';

          checks.tests = rustiq;
        };
    };
}
