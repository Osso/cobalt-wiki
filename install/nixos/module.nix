{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.cobaltWiki;
  packages = cfg.packages;
  account = "cobalt-wiki";
  state = "/var/lib/cobalt-wiki";
  socket = "/run/cobalt-wiki-postgresql";
  postgres = pkgs.postgresql_17;
  python = "${pkgs.python3}/bin/python3";
  marker = "${state}/provisioned";
  # Development deploys (install/dev-deploy.sh) run the apps from a mutable
  # directory instead of the Nix packages.
  app = cfg.appDirectory;
  deepwellBin = if app == null then "${packages.deepwell}/bin/deepwell" else "${app}/deepwell/deepwell";
  deepwellShare = if app == null then "${packages.deepwell}/share/deepwell" else "${app}/deepwell";
  defaults = builtins.fromTOML (builtins.readFile ../../deepwell/config.example.toml);
  runtimeConfig = (pkgs.formats.toml { }).generate "cobalt-deepwell.toml" (
    lib.recursiveUpdate defaults {
      server = {
        address = "127.0.0.1:2747";
        pid-file = "";
      };
      database = {
        run-seeder = false;
        seeder-path =
          if cfg.bootstrapSeedDirectory == null then "${state}/seed" else cfg.bootstrapSeedDirectory;
      };
      domain = {
        main = cfg.mainDomain;
        files = cfg.filesDomain;
      };
      locale.path = "${deepwellShare}/locales";
      email = {
        mock-mailcheck = false;
        automation-address = "noreply@${cfg.mainDomain}";
        notification-address = "notifications@${cfg.mainDomain}";
        newsletter-address = "newsletter@${cfg.mainDomain}";
      };
    }
  );
  backendEnvironment = {
    DATABASE_URL = "postgresql:///cobalt_wiki?host=${socket}&user=${account}";
    REDIS_URL = "redis://127.0.0.1:6381";
    S3_FILES_BUCKET = "cobalt-wiki-files";
    S3_TEXT_BLOCKS_BUCKET = "cobalt-wiki-text-blocks";
    S3_REGION_NAME = if cfg.externalStorage == null then "local" else cfg.externalStorage.region;
    S3_PATH_STYLE = "true";
    S3_CUSTOM_ENDPOINT =
      if cfg.externalStorage == null then "http://127.0.0.1:9000" else cfg.externalStorage.endpoint;
  };
  common = {
    User = account;
    Group = account;
    Slice = "cobalt-wiki.slice";
    UMask = "0077";
    NoNewPrivileges = true;
    PrivateTmp = true;
    StateDirectoryMode = "0700";
    RuntimeDirectoryMode = "0700";
  };
  daemon = common // {
    Restart = "on-failure";
    RestartSec = "5s";
  };
  databaseInit = pkgs.writeText "cobalt-postgresql-init.py" ''
    import pathlib
    import subprocess
    directory = pathlib.Path("/var/lib/cobalt-wiki-postgresql")
    if not (directory / "PG_VERSION").exists():
        subprocess.run([
            "${postgres}/bin/initdb", "-D", str(directory),
            "--username=${account}", "--encoding=UTF8", "--locale=C.UTF-8",
            "--auth-local=peer", "--auth-host=reject",
        ], check=True)
  '';
  databasePrepare = pkgs.writeText "cobalt-postgresql-prepare.py" ''
    import subprocess
    command = ["${postgres}/bin/psql", "--host=${socket}", "--username=${account}", "--dbname=postgres", "--set=ON_ERROR_STOP=1"]
    result = subprocess.run(command + ["--tuples-only", "--no-align", "--command=SELECT 1 FROM pg_database WHERE datname = 'cobalt_wiki'"], check=True, text=True, capture_output=True)
    if result.stdout.strip() != "1":
        subprocess.run(["${postgres}/bin/createdb", "--host=${socket}", "--username=${account}", "--owner=${account}", "cobalt_wiki"], check=True)
    subprocess.run(["${postgres}/bin/psql", "--host=${socket}", "--username=${account}", "--dbname=cobalt_wiki", "--set=ON_ERROR_STOP=1", "--command=CREATE EXTENSION IF NOT EXISTS pgcrypto"], check=True)
  '';
  startStorage = pkgs.writeText "cobalt-silo-start.py" ''
    import os
    for source, target in [("S3_ACCESS_KEY_ID", "MINIO_ROOT_USER"), ("S3_SECRET_ACCESS_KEY", "MINIO_ROOT_PASSWORD")]:
        value = os.environ.get(source)
        if not value:
            raise RuntimeError(f"Required storage credential is missing: {source}")
        os.environ[target] = value
    os.execv("${packages.silo}/bin/silo", ["silo", "server", "--address", "127.0.0.1:9000", "--console-address", "127.0.0.1:9001", "--config-dir", "/var/lib/cobalt-wiki-s3-config", "/var/lib/cobalt-wiki-s3"])
  '';
  prepareBuckets = pkgs.writeText "cobalt-storage-buckets.py" ''
    import os
    import random
    import subprocess
    import time
    import urllib.error
    import urllib.request
    from urllib.parse import quote
    from email.utils import parsedate_to_datetime

    for attempt in range(6):
        try:
            with urllib.request.urlopen("http://127.0.0.1:9000/minio/health/ready", timeout=3):
                break
        except urllib.error.HTTPError as error:
            if error.code not in (429, 500, 502, 503, 504) or attempt == 5:
                raise
            delay = error.headers.get("Retry-After")
            if delay:
                seconds = float(delay) if delay.isdigit() else max(0, parsedate_to_datetime(delay).timestamp() - time.time())
            else:
                seconds = min(2 ** attempt, 8) + random.random()
            time.sleep(seconds)
        except (urllib.error.URLError, TimeoutError):
            if attempt == 5:
                raise
            time.sleep(min(2 ** attempt, 8) + random.random())
    access = quote(os.environ["S3_ACCESS_KEY_ID"], safe="")
    secret = quote(os.environ["S3_SECRET_ACCESS_KEY"], safe="")
    os.environ["MC_HOST_cobalt"] = f"http://{access}:{secret}@127.0.0.1:9000"
    subprocess.run(["${pkgs.minio-client}/bin/mc", "--config-dir", "/run/cobalt-wiki-buckets", "mb", "--ignore-existing", "cobalt/cobalt-wiki-files", "cobalt/cobalt-wiki-text-blocks"], check=True)
  '';
  requireProvisioning = pkgs.writeText "cobalt-require-provisioning.py" ''
    import subprocess
    from pathlib import Path
    if not Path("${marker}").is_file():
        raise RuntimeError("Cobalt database is not provisioned. Apply migrations and reviewed production seeds, then create ${marker}; stock demo seeds must not be used.")
    # Deploys ship new migrations with the package; sqlx applies only pending ones.
    subprocess.run(["${pkgs.sqlx-cli}/bin/sqlx", "migrate", "run", "--source", "${deepwellShare}/migrations"], check=True)
  '';
  bootstrap = pkgs.writeText "cobalt-bootstrap.py" ''
    import os
    import pathlib
    import subprocess
    marker = pathlib.Path("${marker}")
    if marker.exists():
        raise RuntimeError("Cobalt provisioning marker already exists; refusing to reseed")
    subprocess.run(["${pkgs.sqlx-cli}/bin/sqlx", "migrate", "run", "--source", "${deepwellShare}/migrations"], check=True)
    os.environ["DEEPWELL_RUNTIME_ACTION"] = "run-seeder"
    subprocess.run(["${deepwellBin}", "${runtimeConfig}"], check=True)
    marker.touch(mode=0o600, exist_ok=False)
  '';
  dependencies = [
    "cobalt-wiki-postgresql.service"
    "cobalt-wiki-cache.service"
  ]
  ++ lib.optional (cfg.externalStorage == null) "cobalt-wiki-buckets.service";
in
{
  imports = [ ./poc-gateway.nix ];

  options.services.cobaltWiki = {
    enable = lib.mkEnableOption "isolated native Cobalt Wikijump runtime";
    packages = lib.mkOption {
      type = lib.types.attrsOf lib.types.package;
      default = import ./packages.nix { inherit pkgs; };
      description = "Native deepwell, wws, framerail, and silo packages.";
    };
    environmentFile = lib.mkOption {
      type = lib.types.str;
      description = "Absolute runtime-only EnvironmentFile containing S3_ACCESS_KEY_ID and S3_SECRET_ACCESS_KEY and any approved email credentials. Do not override module-owned addresses, database, or bucket settings.";
    };
    mainDomain = lib.mkOption {
      type = lib.types.str;
      description = "Wikijump main-domain configuration supplied by routing integration.";
    };
    filesDomain = lib.mkOption {
      type = lib.types.str;
      description = "Wikijump file-domain configuration supplied by routing integration.";
    };
    externalStorage = lib.mkOption {
      type = lib.types.nullOr (
        lib.types.submodule {
          options = {
            endpoint = lib.mkOption {
              type = lib.types.str;
              description = "S3 endpoint URL, e.g. https://<account>.r2.cloudflarestorage.com.";
            };
            region = lib.mkOption {
              type = lib.types.str;
              default = "auto";
              description = "S3 region name (R2 uses auto).";
            };
          };
        }
      );
      default = null;
      description = "Use an external S3 service instead of the local Silo storage units. Buckets cobalt-wiki-files and cobalt-wiki-text-blocks must exist, and S3_ACCESS_KEY_ID/S3_SECRET_ACCESS_KEY in environmentFile must be its keys.";
    };
    appDirectory = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "/var/lib/cobalt-wiki/app";
      description = "Run Deepwell (deepwell/{deepwell,migrations,locales}) and Framerail (framerail/{build,node_modules,package.json}) from this mutable directory, filled by install/dev-deploy.sh, instead of the Nix packages. For fast iteration only; null deploys the reproducible packages.";
    };
    wikidotSync = {
      enable = lib.mkEnableOption "periodic sync of pages changed on the source Wikidot site";
      origin = lib.mkOption {
        type = lib.types.str;
        example = "https://cobalt-company.wikidot.com";
        description = "Source Wikidot site origin.";
      };
      siteId = lib.mkOption {
        type = lib.types.int;
        description = "Replica site ID receiving the changes.";
      };
      passwordFile = lib.mkOption {
        type = lib.types.str;
        description = "Runtime file (outside the store) with the cobalt-import account password, readable by the service account.";
      };
      interval = lib.mkOption {
        type = lib.types.str;
        default = "*:0/15";
        description = "systemd OnCalendar schedule.";
      };
    };
    bootstrapSeedDirectory = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Private reviewed production seed directory. Enables a manual-only bootstrap unit; never selects bundled demo seeds. With null, provision externally and create /var/lib/cobalt-wiki/provisioned.";
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion =
          lib.hasPrefix "/" cfg.environmentFile && !(lib.hasPrefix "/nix/store/" cfg.environmentFile);
        message = "Cobalt environmentFile must be an absolute runtime path outside the Nix store.";
      }
      {
        assertion =
          cfg.bootstrapSeedDirectory == null
          || (
            lib.hasPrefix "/" cfg.bootstrapSeedDirectory
            && !(lib.hasPrefix "/nix/store/" cfg.bootstrapSeedDirectory)
          );
        message = "Cobalt bootstrap seeds must be reviewed private runtime files, not bundled Nix-store seeds.";
      }
      {
        assertion =
          !cfg.wikidotSync.enable
          || (
            lib.hasPrefix "/" cfg.wikidotSync.passwordFile
            && !(lib.hasPrefix "/nix/store/" cfg.wikidotSync.passwordFile)
          );
        message = "Cobalt wikidotSync.passwordFile must be an absolute runtime path outside the Nix store.";
      }
      {
        assertion = cfg.mainDomain != "" && cfg.filesDomain != "";
        message = "Cobalt main and files domains must be supplied by routing integration.";
      }
    ];
    # A dev Deepwell binary links the Nix package's glibc and libmagic; keep them.
    environment.etc."cobalt-wiki/deepwell-package" = lib.mkIf (app != null) {
      source = packages.deepwell;
    };
    users.groups.${account} = { };
    users.users.${account} = {
      isSystemUser = true;
      group = account;
    };
    systemd.timers.cobalt-wiki-wikidot-sync = lib.mkIf cfg.wikidotSync.enable {
      wantedBy = [ "timers.target" ];
      timerConfig = {
        OnCalendar = cfg.wikidotSync.interval;
        Persistent = true;
      };
    };
    systemd.slices.cobalt-wiki = {
      description = "Cobalt wiki resource budget";
      sliceConfig = {
        MemoryAccounting = true;
        MemoryMax = "2G";
      };
    };
    systemd.services =
      lib.optionalAttrs (cfg.externalStorage == null) {
      cobalt-wiki-storage = {
        description = "Cobalt private S3 storage";
        wantedBy = [ "multi-user.target" ];
        environment.MINIO_REGION_NAME = "local";
        serviceConfig = daemon // {
          StateDirectory = [
            "cobalt-wiki-s3"
            "cobalt-wiki-s3-config"
          ];
          EnvironmentFile = cfg.environmentFile;
          ExecStart = "${python} ${startStorage}";
        };
      };
      cobalt-wiki-buckets = {
        description = "Initialize Cobalt S3 buckets";
        requires = [ "cobalt-wiki-storage.service" ];
        after = [ "cobalt-wiki-storage.service" ];
        serviceConfig = common // {
          Type = "oneshot";
          RemainAfterExit = true;
          RuntimeDirectory = "cobalt-wiki-buckets";
          EnvironmentFile = cfg.environmentFile;
          ExecStart = "${python} ${prepareBuckets}";
        };
      };
      }
      // {
      cobalt-wiki-wikidot-sync = lib.mkIf cfg.wikidotSync.enable {
        description = "Sync pages changed on Wikidot into the Cobalt replica";
        requires = [ "cobalt-wiki-deepwell.service" ];
        after = [ "cobalt-wiki-deepwell.service" "network-online.target" ];
        wants = [ "network-online.target" ];
        environment.PYTHONPATH = "${packages.wikidot-tools}/lib/cobalt";
        serviceConfig = common // {
          Type = "oneshot";
          RuntimeDirectory = "cobalt-wiki-wikidot-sync";
          ExecStart = lib.escapeShellArgs [
            python
            "${./wikidot_sync.py}"
            cfg.wikidotSync.origin
            (toString cfg.wikidotSync.siteId)
            cfg.wikidotSync.passwordFile
            "${postgres}/bin/psql --host=${socket} --username=${account} --dbname=cobalt_wiki"
            "http://127.0.0.1:2747/jsonrpc"
          ];
        };
      };
      cobalt-wiki-postgresql = {
        description = "Cobalt private PostgreSQL 17";
        wantedBy = [ "multi-user.target" ];
        serviceConfig = daemon // {
          StateDirectory = "cobalt-wiki-postgresql";
          RuntimeDirectory = "cobalt-wiki-postgresql";
          ExecStartPre = "${python} ${databaseInit}";
          ExecStart = "${postgres}/bin/postgres -D /var/lib/cobalt-wiki-postgresql -k ${socket} -h '' -c unix_socket_permissions=0700 -c shared_buffers=128MB -c max_connections=110";
          ExecStartPost = "${python} ${databasePrepare}";
          Type = "notify";
        };
      };
      cobalt-wiki-cache = {
        description = "Cobalt private Valkey";
        wantedBy = [ "multi-user.target" ];
        serviceConfig = daemon // {
          StateDirectory = "cobalt-wiki-cache";
          ExecStart = "${pkgs.valkey}/bin/valkey-server --bind 127.0.0.1 --port 6381 --protected-mode yes --dir /var/lib/cobalt-wiki-cache --appendonly yes --daemonize no";
        };
      };
      cobalt-wiki-deepwell = {
        description = "Cobalt Wikijump backend";
        wantedBy = [ "multi-user.target" ];
        requires = dependencies;
        after = dependencies;
        environment = backendEnvironment;
        serviceConfig = daemon // {
          StateDirectory = "cobalt-wiki";
          EnvironmentFile = cfg.environmentFile;
          ExecStartPre = "${python} ${requireProvisioning}";
          ExecStart = "${deepwellBin} ${runtimeConfig}";
          ExecStartPost = "${python} ${./wait_deepwell.py}";
        };
      };
      cobalt-wiki-framerail = {
        description = "Cobalt production SvelteKit server";
        wantedBy = [ "multi-user.target" ];
        requires = [ "cobalt-wiki-deepwell.service" ];
        after = [ "cobalt-wiki-deepwell.service" ];
        environment = {
          NODE_ENV = "production";
          HOST = "127.0.0.1";
          PORT = "3393";
          ORIGIN = "https://${cfg.mainDomain}";
          DEEPWELL_HOST = "127.0.0.1";
          DEEPWELL_PORT = "2747";
        };
        serviceConfig = daemon // {
          ExecStart =
            if app == null then
              "${packages.framerail}/bin/framerail"
            else
              "${pkgs.nodejs_22}/bin/node ${app}/framerail/build/index.js";
        };
      };
      cobalt-wiki-wws = {
        description = "Cobalt Wikijump file server";
        wantedBy = [ "multi-user.target" ];
        requires = dependencies ++ [ "cobalt-wiki-deepwell.service" ];
        after = dependencies ++ [ "cobalt-wiki-deepwell.service" ];
        environment = backendEnvironment // {
          ADDRESS = "127.0.0.1:3466";
          DEEPWELL_URL = "http://127.0.0.1:2747";
        };
        serviceConfig = daemon // {
          EnvironmentFile = cfg.environmentFile;
          ExecStart = "${packages.wws}/bin/wws";
        };
      };
    }
    // lib.optionalAttrs (cfg.bootstrapSeedDirectory != null) {
      cobalt-wiki-bootstrap = {
        description = "Manually provision Cobalt from reviewed production seeds";
        requires = dependencies;
        after = dependencies;
        environment = backendEnvironment;
        serviceConfig = common // {
          Type = "oneshot";
          StateDirectory = "cobalt-wiki";
          EnvironmentFile = cfg.environmentFile;
          ExecStart = "${python} ${bootstrap}";
        };
      };
    };
  };
}
