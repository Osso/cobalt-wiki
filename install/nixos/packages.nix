{ pkgs, craneLib }:
let
  inherit (pkgs) lib;
  root = ../..;
  nodejs = pkgs.nodejs_22;
  pnpm = pkgs.pnpm.override { inherit nodejs; };
  version = "2026.8.20";

  # Isolated toolchain: Silo requires Go 1.27.1, newer than the host pin.
  siloGo = pkgs.go_1_26.overrideAttrs (old: {
    version = "1.27.1";
    src = pkgs.fetchurl {
      url = "https://go.dev/dl/go1.27.1.src.tar.gz";
      sha256 = "4e408abae126d916b6164627193f2c54f0e3ca1312d693b86db45f862ab238b1";
    };
    patches = map (
      patch:
      if builtins.baseNameOf (toString patch) == "go_no_vendor_checks-1.26.patch" then
        pkgs.writeText "go_no_vendor_checks-1.27.patch" (
          builtins.replaceStrings [ "loaderstate" ] [ "ld" ] (builtins.readFile patch)
        )
      else
        patch
    ) old.patches;
    env = old.env // {
      GOROOT_BOOTSTRAP = "${pkgs.go_1_26}/share/go";
    };
  });

  silo = (pkgs.buildGoModule.override { go = siloGo; }) {
    pname = "silo";
    version = "2026-09-16";
    src = pkgs.fetchzip {
      url = "https://github.com/pgsty/silo/archive/2a4d51406b7ed87af5fe6fe0f801f3290f96eb3c.tar.gz";
      hash = "sha256-M9sBb2pFY00kYCUBk2ctWEM4xraEXdGcaEgvzQna+9w=";
    };
    vendorHash = "sha256-STpltATG8UVhJMuUn3NeNOpHLv3jBdtyheB0jQ28qjY=";
    subPackages = [ "." ];
    # Storage integration tests belong to the configured runtime gate.
    doCheck = false;
    postInstall = ''
      mv "$out/bin/minio" "$out/bin/silo"
    '';
    meta = {
      description = "Pinned Silo S3 server for native Wikijump storage";
      homepage = "https://github.com/pgsty/silo";
      license = lib.licenses.agpl3Plus;
      platforms = [ "x86_64-linux" ];
      mainProgram = "silo";
    };
  };

  deepwellArgs = {
    pname = "cobalt-deepwell";
    inherit version;
    src = root + /deepwell;
    strictDeps = true;
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ pkgs.file ];
    # Fat LTO links single-threaded for minutes; thin LTO keeps deploys fast.
    CARGO_PROFILE_RELEASE_LTO = "thin";
    # Database/S3 integration tests require a running configured instance.
    doCheck = false;
  };

  deepwell = craneLib.buildPackage (
    deepwellArgs
    // {
      cargoArtifacts = craneLib.buildDepsOnly deepwellArgs;
      postInstall = ''
        mkdir -p "$out/share/deepwell"
        cp -r migrations seeder "$out/share/deepwell/"
        cp config.example.toml "$out/share/deepwell/"
        cp -r ${root + /locales} "$out/share/deepwell/locales"
      '';
      meta = {
        description = "Wikijump backend with runtime data assets";
        license = lib.licenses.agpl3Plus;
        platforms = [ "x86_64-linux" ];
        mainProgram = "deepwell";
      };
    }
  );

  wws = pkgs.rustPlatform.buildRustPackage {
    pname = "cobalt-wws";
    inherit version;
    src = root + /wws;
    cargoLock.lockFile = root + /wws/Cargo.lock;
    doCheck = false;
    meta = {
      description = "Wikijump file and generated-content server";
      license = lib.licenses.agpl3Plus;
      platforms = [ "x86_64-linux" ];
      mainProgram = "wws";
    };
  };

  framerailSource = lib.fileset.toSource {
    root = root;
    fileset = lib.fileset.unions [
      (root + /framerail)
      (root + /assets)
    ];
  };

  framerail = pkgs.stdenvNoCC.mkDerivation {
    pname = "cobalt-framerail";
    version = "2026.3.25";
    src = framerailSource;
    sourceRoot = "source/framerail";
    nativeBuildInputs = [
      nodejs
      pnpm
      pkgs.pnpmConfigHook
      pkgs.makeWrapper
    ];
    pnpmDeps = pkgs.fetchPnpmDeps {
      inherit pnpm;
      pname = "cobalt-framerail";
      version = "2026.3.25";
      src = root + /framerail;
      fetcherVersion = 4;
      hash = "sha256-5jo+F84E9DurMeMGJarO/WN77EL/QZCi4BQ3JHWNn6Y=";
    };
    buildPhase = ''
      runHook preBuild
      pnpm run build
      runHook postBuild
    '';
    installPhase = ''
      runHook preInstall
      mkdir -p "$out/share/framerail" "$out/bin"
      cp -r build node_modules package.json "$out/share/framerail/"
      makeWrapper ${nodejs}/bin/node "$out/bin/framerail" \
        --add-flags "$out/share/framerail/build/index.js"
      runHook postInstall
    '';
    meta = {
      description = "Production SvelteKit adapter-node server for Wikijump";
      license = lib.licenses.agpl3Plus;
      platforms = [ "x86_64-linux" ];
      mainProgram = "framerail";
    };
  };
in
{
  inherit
    deepwell
    wws
    framerail
    silo
    ;
}
