{
  pkgs,
  ...
}:
let
  inherit (pkgs) lib;
in
pkgs.rustPlatform.buildRustPackage {
  pname = "nix-wire";
  version = "0.1.0";
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../crates
    ];
  };
  cargoLock.lockFile = ../Cargo.lock;

  meta = with pkgs.lib; {
    description = "A collection of tools for the Nix daemon wire protocol";
    homepage = "https://github.com/zimbatm/nix-wire";
    license = licenses.mit;
    mainProgram = "nix-wire-decode";
  };
}
