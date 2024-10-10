{
  description = "SyncDisBoi";

  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    naersk,
    fenix,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [fenix.overlays.default];
        };

        toolchain = with fenix.packages.${system};
          combine [
            minimal.rustc
            minimal.cargo
            targets.x86_64-unknown-linux-musl.latest.rust-std
            targets.x86_64-pc-windows-gnu.latest.rust-std
            targets.aarch64-unknown-linux-musl.latest.rust-std
          ];

        naersk' = naersk.lib.${system}.override {
          cargo = toolchain;
          rustc = toolchain;
        };

        naerskBuildPackage = args:
          naersk'.buildPackage (
            args
            // cargoConfig
          );
        naerskBuildPackageT = target: args: naerskBuildPackage (args // {CARGO_BUILD_TARGET = target;});

        cargoConfig = {
          # Tells Cargo to enable static compilation.
          # (https://doc.rust-lang.org/cargo/reference/config.html#targettriplerustflags)
          #
          # Note that the resulting binary might still be considered dynamically
          # linked by ldd, but that's just because the binary might have
          # position-independent-execution enabled.
          # (see: https://github.com/rust-lang/rust/issues/79624#issuecomment-737415388)
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_RUSTFLAGS = "-C target-feature=+crt-static";

          # Tells Cargo that it should use Wine to run tests.
          # (https://doc.rust-lang.org/cargo/reference/config.html#targettriplerunner)
          CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER = pkgs.writeScript "wine-wrapper" ''
            export WINEPREFIX="$(mktemp -d)"
            exec wine64 $@
          '';
        };

        fnBuildInputs = pkgs: with pkgs; [openssl];
        shellPkgs = with pkgs; [
          wineWowPackages.stable
        ];
      in rec {
        defaultPackage = naerskBuildPackage {
          src = ./.;
          buildInputs =
            (fnBuildInputs pkgs)
            ++ (with pkgs;
              lib.optionals stdenv.isDarwin
              [
                darwin.apple_sdk.frameworks.CoreFoundation
                darwin.apple_sdk.frameworks.CoreServices
                darwin.apple_sdk.frameworks.SystemConfiguration
                darwin.apple_sdk.frameworks.Security
              ]);
        };

        packages.x86_64-unknown-linux-musl = naerskBuildPackageT "x86_64-unknown-linux-musl" {
          src = ./.;
          nativeBuildInputs = with pkgs; [pkgsStatic.stdenv.cc];
          buildInputs = fnBuildInputs pkgs.pkgsCross.musl64.pkgsStatic;
        };

        packages.aarch64-unknown-linux-musl = with pkgs.pkgsCross.aarch64-multiplatform-musl;
          naerskBuildPackageT "aarch64-unknown-linux-musl" {
            src = ./.;
            strictDeps = true;
            depsBuildBuild = [
              stdenv.cc
            ];
            "CC_aarch64_unknown_linux_musl" = "${stdenv.cc}/bin/${stdenv.cc.targetPrefix}cc";
            "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER" = "${stdenv.cc}/bin/${stdenv.cc.targetPrefix}cc";
            buildInputs = fnBuildInputs pkgsStatic;
          };

        packages.x86_64-pc-windows-gnu = naerskBuildPackageT "x86_64-pc-windows-gnu" {
          src = ./.;
          strictDeps = true;
          depsBuildBuild = with pkgs; [
            pkgsCross.mingwW64.stdenv.cc
            pkgsCross.mingwW64.windows.pthreads
          ];
          nativeBuildInputs = with pkgs; [
            # We need Wine to run tests:
            wineWowPackages.stable
          ];
          buildInputs = fnBuildInputs pkgs.pkgsCross.mingwW64;
        };

        devShell = pkgs.mkShell (
          {
            inputsFrom = [defaultPackage];
            packages = shellPkgs;
            #CARGO_BUILD_TARGET = "x86_64-unknown-linux-gnu";
            LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath (fnBuildInputs pkgs)}";
          }
          // cargoConfig
        );
      }
    );
}
