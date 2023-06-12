{pkgs ? import <nixpkgs> {} }:

pkgs.rustPlatform.buildRustPackage rec {
  pname = "deplorable";
  version = "0.1.1";
  src = ./.;
  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  buildInputs = [ pkgs.openssl ];
  nativeBuildInputs = [ pkgs.pkg-config ];

  meta = {
    description = "A simple & small daemon to deploy static website and other code from GitHub webhooks";
    homepage = "https://github.com/alevy/deplorable";
    license = pkgs.lib.licenses.gpl3;
  };
}
