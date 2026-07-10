{ system ? builtins.currentSystem
}:

let
  pins = import ./npins;
  pkgs = import pins.nixpkgs { inherit system; };
in pkgs.mkShell {
  packages = with pkgs; [
    rustc
    cargo
    clippy
  ];
}
