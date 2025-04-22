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
            default.rustc
            default.cargo
            default.clippy
            default.rustfmt
            pkgs.rust-analyzer
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
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_RUSTFLAGS = "-C target-feature=+crt-static";
        };

        fnBuildInputs = pkgs:
          with pkgs; [
            pkg-config
            openssl
          ];
        shellPkgs = with pkgs; [];
      in rec {
        defaultPackage = naerskBuildPackage {
          src = ./.;
          #doCheck = true;
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
          #doCheck = true;
          nativeBuildInputs = with pkgs; [pkgsStatic.stdenv.cc];
          buildInputs = fnBuildInputs pkgs.pkgsCross.musl64.pkgsStatic;
        };

        packages.aarch64-unknown-linux-musl = with pkgs.pkgsCross.aarch64-multiplatform-musl;
          naerskBuildPackageT "aarch64-unknown-linux-musl" {
            src = ./.;
            strictDeps = true;
            #doCheck = true;
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
          #doCheck = true;
          depsBuildBuild = with pkgs; [
            pkgsCross.mingwW64.stdenv.cc
            pkgsCross.mingwW64.windows.pthreads
          ];
          nativeBuildInputs = with pkgs; [];
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
