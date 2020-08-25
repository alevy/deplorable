{ pkgs ? import <nixpkgs> {} }:

with pkgs;
stdenv.mkDerivation {
  name = "deplorable-site";
  buildInputs = [ hugo ];
  builder = writeText "builder.sh" ''
    source ${stdenv}/setup
    HUGO_RESOURCEDIR=$TMP hugo -e production -d $out -s $src
    '';
  src = ./.;
}
