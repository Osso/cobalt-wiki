{ nodePkgs }:
let
  # Pin shared Brotli until pinned nodePkgs provides the fixed linked decoder.
  brotli = nodePkgs.brotli.overrideAttrs (_: {
    version = "1.2.1";
    src = nodePkgs.fetchFromGitHub {
      owner = "google";
      repo = "brotli";
      tag = "v1.2.1";
      hash = "sha256-llY72cYffDv0ugsH0p89xIPEbElV4L8u7U/Fe8hT3gI=";
    };
  });
  nodejs-slim = nodePkgs.nodejs-slim_26.override {
    callPackage = nodePkgs.lib.callPackageWith (nodePkgs // { inherit brotli; });
  };
in
nodePkgs.nodejs_26.override { inherit nodejs-slim; }
