{ bashInteractive
, buildEnv
, clam
, gitMinimal
, nix2container
, runCommand
, name ? "xfnw/clam"
, tag ? "latest"
, maxLayers ? 50
}:

let
  rootEnv = buildEnv {
    name = "root";
    paths = [
      bashInteractive
      clam
      gitMinimal
    ];
    pathsToLink = [
      "/bin"
    ];
  };
  dataDir = runCommand "dataDir" { } ''
    mkdir -p $out/data
  '';
in
nix2container.buildImage {
  inherit name tag maxLayers;

  copyToRoot = [
    rootEnv
    dataDir
  ];

  perms = [
    {
      path = dataDir;
      regex = "/data";
      mode = "0755";
      uid = 1000;
      gid = 1000;
    }
  ];

  config = {
    Cmd = [
      "git" "daemon" "--export-all" "--reuseaddr" "--base-path=/data"
    ];
    User = "1000:1000";
    WorkingDir = "/data";
  };
}
