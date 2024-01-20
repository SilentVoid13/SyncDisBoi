{pkgs ? import <nixpkgs> {}}:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
      openssl
      pkg-config
      python311Packages.ytmusicapi
  ];
}
