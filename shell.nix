{ pkgs ? import <nixpkgs> {} }:

with pkgs;
let
  crate2nix = import (fetchFromGitHub {
    owner = "kolloch";
    repo = "crate2nix";
    rev = "0.8.0";
    sha256 = "17mmf5sqn0fmpqrf52icq92nf1sy5yacwx9vafk43piaq433ba56";
  }) {};
in mkShell {
  buildInputs = [
    rustc
    cargo
    crate2nix
    pkgconfig
    openssl
  ];
}
