{ system ? builtins.currentSystem }:

let
  pins = import ./npins;
  pkgs = import pins.nixpkgs { inherit system; };
in pkgs.mkShell {
  packages = with pkgs; [
    rustc
    cargo
    clippy
    (npins.override {
      # nix-prefetch-docker depends on c++ nix, and we are
      # not using any container dependencies anyways
      nix-prefetch-docker = null;
    })
  ];
}
