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
              ruff-format = {
                enable = true;
                package = pythonSet.ruff;
              };
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
              relativePath == "clippy.toml"
              || builtins.elem relativePath [
                "COPYING"
                "LICENSE-MIT"
                "LICENSE-APACHE"
                "DISTRIBUTION.md"
                "THIRD-PARTY-NOTICES.txt"
                "THIRD-PARTY-NOTICES.sha256"
              ]
              || craneLib.filterCargoSources path type
              || inProjectTree "assets"
              || inProjectTree "build"
              || builtins.elem relativePath [
                ".cargo/config"
                ".cargo/config.toml"
              ]
              || inProjectTree "third-party"
              || inProjectTree "samples"
              || inProjectTree "schemas"
              || inProjectTree "src/runfile/nickel"
              || inProjectTree "crates/rustiq-core/build"
              || inProjectTree "tests/data"
              || inProjectTree "crates/rustiq-core/tests/data";
          };

          commonCargoArgs = {
            pname = "rustiq";
            version = cargoToml.package.version;
            src = cargoSource;
            strictDeps = true;
            cargoExtraArgs = "--locked --workspace --no-default-features --features online";
          };

          cargoArtifacts = craneLib.buildDepsOnly commonCargoArgs;

          # Generate installed assets with a separate executable so the shipped
          # CLI does not retain the optional generator code or dependencies.
          cliGenerators = craneLib.buildPackage (
            commonCargoArgs
            // {
              inherit cargoArtifacts;
              cargoExtraArgs = "${commonCargoArgs.cargoExtraArgs} --features manpages,completions";
              doCheck = false;
              nativeBuildInputs = [ pkgs.git ];
            }
          );

          rustiq = craneLib.buildPackage (
            commonCargoArgs
            // {
              inherit cargoArtifacts;
              nativeBuildInputs = [
                pkgs.git
                pkgs.installShellFiles
              ];
              # Crane's package build uses Cargo's release profile. Pass the
              # exact profile name because PROFILE is only debug/release-like.
              RUSTIQ_BUILD_PROFILE = "release";
              RUSTIQ_BUILD_SYSTEM = "Nix";
              RUSTIQ_BUILD_GIT_COMMIT = pkgs.lib.removeSuffix "-dirty" (
                inputs.self.rev or inputs.self.dirtyRev or "unknown"
              );
              RUSTIQ_BUILD_GIT_STATE =
                if inputs.self ? rev then
                  "clean"
                else if inputs.self ? dirtyRev then
                  "dirty"
                else
                  "unknown";
              postInstall = ''
                export RUSTIQ_PACKAGING=true
                ${cliGenerators}/bin/rustiq manpages manpages
                installManPage manpages/*.1
                mkdir -p completions "$out/share/rustiq/completions"
                for shell in bash zsh fish elvish powershell nushell; do
                  ${cliGenerators}/bin/rustiq completions "$shell" > "completions/$shell"
                done
                installShellCompletion --cmd rustiq \
                  --bash completions/bash --zsh completions/zsh --fish completions/fish
                cp completions/nushell "$out/share/rustiq/completions/rustiq.nu"
                cp completions/powershell "$out/share/rustiq/completions/rustiq.ps1"
                cp completions/elvish "$out/share/rustiq/completions/rustiq.elv"
                mkdir -p "$out/share/doc/rustiq"
                cp COPYING LICENSE-MIT LICENSE-APACHE DISTRIBUTION.md THIRD-PARTY-NOTICES.txt \
                  "$out/share/doc/rustiq/"
                cp -r ${book}/. "$out/share/doc/rustiq/"
              '';
              doInstallCheck = true;
              installCheckPhase = ''
                runHook preInstallCheck
                "$out/bin/rustiq" --help > installed-help
                ! grep -Eq 'manpages|completions' installed-help
                test -s "$out/share/man/man1/rustiq.1.gz"
                test "$("$out/bin/rustiq" docs --print-path)" = "$out/share/doc/rustiq/index.html"
                test -s "$out/share/doc/rustiq/mathjax/MathJax.js"
                test "$("$out/bin/rustiq" docs --web --print-path)" = "https://mveril.github.io/RustiQ/"
                test -s "$out/share/man/man1/rustiq-artifact-inspect.1.gz"
                test -s "$out/share/bash-completion/completions/rustiq.bash"
                test -s "$out/share/zsh/site-functions/_rustiq"
                test -s "$out/share/fish/vendor_completions.d/rustiq.fish"
                for file in rustiq.nu rustiq.ps1 rustiq.elv; do
                  test -s "$out/share/rustiq/completions/$file"
                done
                runHook postInstallCheck
              '';
              meta.license = with pkgs.lib.licenses; [
                mit
                asl20
                lgpl3Only
                lgpl3Plus
                mpl20
              ];
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
            cargo-about
            cargo-dist
            cargo-info

            bacon
            cargo-watch
            git
            hyperfine
            jq
            mdbook
            mdbook-mermaid
            nixd
            nixfmt
            nickel
            nls
            ripgrep
            time
          ];

          rustBuildPlatformPackages = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.libiconv ];

          rustDevelopmentPlatformPackages =
            pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux (
              with pkgs;
              [
                clang
                cargo-flamegraph
                gdb
                inferno
                perf
              ]
            )
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin (
              with pkgs;
              [
                samply
              ]
            );

          minimalRustPackages = rustBuildPackages ++ rustBuildPlatformPackages;

          completeRustPackages =
            minimalRustPackages ++ rustDevelopmentPackages ++ rustDevelopmentPlatformPackages;

          pythonDevelopmentPackages = with pkgs; [ uv ];

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
          }
          // pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
            # uv installs binary Python wheels into .venv. Unlike Nix-built
            # extensions, their ELF dependencies are not patched, so make the
            # C++ runtime available to NumPy at import time.
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.stdenv.cc.cc.lib ];
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

              RUSTIQ_BIN="${rustiq}/bin/rustiq" exec pytest "$reference_tests" "$@"
            '';
          };

          book =
            pkgs.runCommand "rustiq-book"
              {
                src = pkgs.lib.fileset.toSource {
                  root = sourceRoot;
                  fileset = pkgs.lib.fileset.unions [
                    ./DATA_SOURCES.md
                    ./DISTRIBUTION.md
                    ./docs/book.toml
                    ./docs/src
                    ./samples/h2
                    ./samples/oh
                    ./tools/check-book-links.py
                    ./tools/test_book_links.py
                    ./tools/build-manual.py
                  ];
                };
                nativeBuildInputs = [
                  pkgs.mdbook
                  pkgs.mdbook-mermaid
                  pkgs.python3
                  pkgs.python3Packages.pytest
                  rustToolchain
                ];
                mathjaxArchive = pkgs.fetchurl {
                  url = "https://registry.npmjs.org/mathjax/-/mathjax-2.7.9.tgz";
                  sha256 = "7131e739848edc14aa661a5516995866b81a477fab8b039d7cc324930e71f786";
                };
              }
              ''
                cp -r "$src" source
                chmod -R u+w source
                cd source
                python tools/build-manual.py --mathjax-archive "$mathjaxArchive"
                test -n "$(find docs/book -maxdepth 1 -name 'mermaid-*.min.js' -print -quit)"
                test -n "$(find docs/book -maxdepth 1 -name 'mermaid-init-*.js' -print -quit)"
                grep -q 'class="mermaid"' docs/book/user-guide/calculation-inputs.html
                mdbook test docs
                pytest tools/test_book_links.py
                python tools/check-book-links.py docs/book --config docs/book.toml --offline
                mv docs/book "$out"
              '';
        in
        {
          packages = {
            default = rustiq;
            inherit book;
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
              cargoExtraArgs = "--locked --workspace --all-features";
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );

          checks.unit-tests = craneLib.cargoTest (
            commonCargoArgs
            // {
              inherit cargoArtifacts;
              cargoExtraArgs = "--locked --workspace --all-features";
              nativeBuildInputs = [ pkgs.git ];
            }
          );
          checks.book = book;
        };
    };
}
