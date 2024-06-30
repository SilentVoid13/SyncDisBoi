{
  description = "SyncDisBoi";

  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    flakebox = {
      url = "github:SilentVoid13/flakebox";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    nixpkgs,
    flake-utils,
    flakebox,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {inherit system;};

        shellPackages = with pkgs; [
          python311Packages.ytmusicapi
        ];

        flakeboxLib = flakebox.lib.${system} {
          config = {
            github.ci.enable = false;
            just.enable = false;
            convco.enable = false;
            git.commit-msg.enable = false;
            git.commit-template.enable = false;
            git.pre-commit.enable = false;

            env.shellPackages = shellPackages;
          };
        };
        project_name = "SyncDisBoi";

        buildPaths = [
          "Cargo.toml"
          "Cargo.lock"
          "src"
        ];

        buildInputs = pkgs:
          with pkgs; [openssl];

        nativeBuildInputs = pkgs:
          with pkgs; [
            pkgsBuildTarget.pkg-config
          ];

        buildSrc = flakeboxLib.filterSubPaths {
          root = builtins.path {
            name = project_name;
            path = ./.;
          };
          paths = buildPaths;
        };

        multiBuild =
          (flakeboxLib.craneMultiBuild {
            inherit buildInputs;
            inherit nativeBuildInputs;
          }) (craneLib': let
            craneLib = craneLib'.overrideArgs {
              pname = project_name;
              src = buildSrc;
            };
          in {
            ${project_name} = craneLib.buildPackage {};
          });
      in {
        packages.default = multiBuild.${project_name};

        legacyPackages = multiBuild;

        devShells = flakeboxLib.mkShells {
          buildInputs = buildInputs pkgs;
          nativeBuildInputs = nativeBuildInputs pkgs;
        };
      }
    );
}
