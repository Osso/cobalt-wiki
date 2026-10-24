{ pkgs }:
let
  # Node uses shared Brotli, so updating Node alone leaves CVE-2026-111497 exposed.
  # Remove these pins when the normal pinned nixpkgs provides both fixed versions.
  brotli = pkgs.brotli.overrideAttrs (old: {
    version = "1.2.1";
    src = pkgs.fetchFromGitHub {
      owner = "google";
      repo = "brotli";
      tag = "v1.2.1";
      hash = "sha256-llY72cYffDv0ugsH0p89xIPEbElV4L8u7U/Fe8hT3gI=";
    };
  });
  nodejs-slim =
    (pkgs.callPackage (pkgs.path + "/pkgs/development/web/nodejs/nodejs.nix") {
      inherit brotli;
      python = pkgs.python3;
    })
      {
        version = "22.23.4";
        sha256 = "7b9adc82d612d0a2a863d96c46de2292552e250cee51956ebbe0146c42aacae9";
        # Node 22.23.4 includes these backports; match the current nixpkgs recipe.
        patches = pkgs.lib.filter (
          patch:
          !pkgs.lib.any (name: pkgs.lib.hasInfix name (toString patch)) [
            "deprecate-http2-priority-signaling.patch"
            "a63126409ad4334dd5d838c39806f38c020748b9"
            "ecbc22dc3709290dcaadf634a28d8307a75952ee"
            "4a32c00fb8dbe55c3bcf9ef43343968c9fe449e6"
          ]
        ) pkgs.nodejs-slim_22.patches;
      };
in
pkgs.nodejs_22.override { inherit nodejs-slim; }
