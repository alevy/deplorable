{ pkgs ? import <nixpkgs> {} }:

with pkgs;
stdenv.mkDerivation {
  name = "deplorable-site";
  buildInputs = [ hugo ];
  builder = writeText "builder.sh" ''
    source ${stdenv}/setup
    hugo -d $out -s $src
    '';
  src = ./.;
}
