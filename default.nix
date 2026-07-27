{ system ? builtins.currentSystem
, pins ? import ./npins
, pkgs ? import pins.nixpkgs { inherit system; }
, crane ? import pins.crane { inherit pkgs; }
, nix2container ? import pins.nix2container { inherit pkgs; }
}:

pkgs.lib.fix (self: {

  clam = pkgs.callPackage ./package.nix { inherit crane; };

  dockerImage = pkgs.callPackage ./docker.nix {
    inherit (nix2container) nix2container;
    inherit (self) clam;
  };

})
