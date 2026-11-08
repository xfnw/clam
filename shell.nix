{ system ? builtins.currentSystem }:

let
  pins = import ./npins;
  pkgs = import pins.nixpkgs { inherit system; };
  vulppkgs = import pins.vulppkgs { inherit pkgs; };
in pkgs.mkShell {
  packages = with pkgs; [
    rustc
    cargo
    clippy
    vulppkgs.npinsMinimal
  ];
}
