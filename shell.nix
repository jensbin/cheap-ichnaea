{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  packages = [
    pkgs.cargo
    pkgs.pkg-config
    pkgs.openssl
  ];
}
