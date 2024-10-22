#with ( import <nixpkgs> {});
#{ nodeEnv, fetchgit, pkgs ? import <nixpkgs> {} }:
{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = [
    pkgs.cargo
    pkgs.pkg-config
    pkgs.openssl
  ];

  shellHook = ''
    echo "cat countries.json | jq '.[] | {iso2, latitude, longitude}' | jq -c -s . > countries_opt.json"
  '';
}
