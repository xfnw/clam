{ system ? builtins.currentSystem
, pins ? import ./npins
, pkgs ? import pins.nixpkgs { inherit system; }
, naersk ? pkgs.callPackage "${pins.naersk}" { } # npins#242
}:

{

  clam = pkgs.callPackage ./package.nix { inherit naersk; };

}
