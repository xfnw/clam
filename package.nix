{ naersk, lib }:

naersk.buildPackage {
  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      ./Cargo.lock
      ./Cargo.toml
      ./src
      ./templates
    ];
  };
}
