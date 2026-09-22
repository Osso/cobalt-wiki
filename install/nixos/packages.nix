{ pkgs }:
let
  inherit (pkgs) lib;
  root = ../..;
  nodejs = pkgs.nodejs_22;
  pnpm = pkgs.pnpm.override { inherit nodejs; };
  version = "2026.8.20";

  deepwell = pkgs.rustPlatform.buildRustPackage {
    pname = "cobalt-deepwell";
    inherit version;
    src = root + /deepwell;
    cargoLock.lockFile = root + /deepwell/Cargo.lock;
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ pkgs.file ];
    # Database/S3 integration tests require a running configured instance.
    doCheck = false;
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
  };

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
  inherit deepwell wws framerail;
}
