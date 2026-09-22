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
      locale.path = "${packages.deepwell}/share/deepwell/locales";
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
    S3_REGION_NAME = "local";
    S3_PATH_STYLE = "true";
    S3_CUSTOM_ENDPOINT = "http://127.0.0.1:9000";
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
  startStorage = pkgs.writeText "cobalt-minio-start.py" ''
    import os
    for source, target in [("S3_ACCESS_KEY_ID", "MINIO_ROOT_USER"), ("S3_SECRET_ACCESS_KEY", "MINIO_ROOT_PASSWORD")]:
        value = os.environ.get(source)
        if not value:
            raise RuntimeError(f"Required storage credential is missing: {source}")
        os.environ[target] = value
    os.execv("${pkgs.minio}/bin/minio", ["minio", "server", "--address", "127.0.0.1:9000", "--console-address", "127.0.0.1:9001", "--config-dir", "/var/lib/cobalt-wiki-s3-config", "/var/lib/cobalt-wiki-s3"])
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
    from pathlib import Path
    if not Path("${marker}").is_file():
        raise RuntimeError("Cobalt database is not provisioned. Apply migrations and reviewed production seeds, then create ${marker}; stock demo seeds must not be used.")
  '';
  bootstrap = pkgs.writeText "cobalt-bootstrap.py" ''
    import os
    import pathlib
    import subprocess
    marker = pathlib.Path("${marker}")
    if marker.exists():
        raise RuntimeError("Cobalt provisioning marker already exists; refusing to reseed")
    subprocess.run(["${pkgs.sqlx-cli}/bin/sqlx", "migrate", "run", "--source", "${packages.deepwell}/share/deepwell/migrations"], check=True)
    os.environ["DEEPWELL_RUNTIME_ACTION"] = "run-seeder"
    subprocess.run(["${packages.deepwell}/bin/deepwell", "${runtimeConfig}"], check=True)
    marker.touch(mode=0o600, exist_ok=False)
  '';
  dependencies = [
    "cobalt-wiki-postgresql.service"
    "cobalt-wiki-cache.service"
    "cobalt-wiki-buckets.service"
  ];
in
{
  options.services.cobaltWiki = {
    enable = lib.mkEnableOption "isolated native Cobalt Wikijump runtime";
    packages = lib.mkOption {
      type = lib.types.attrsOf lib.types.package;
      default = import ./packages.nix { inherit pkgs; };
      description = "Native deepwell, wws, and framerail packages.";
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
        assertion = cfg.mainDomain != "" && cfg.filesDomain != "";
        message = "Cobalt main and files domains must be supplied by routing integration.";
      }
    ];
    users.groups.${account} = { };
    users.users.${account} = {
      isSystemUser = true;
      group = account;
    };
    systemd.slices.cobalt-wiki = {
      description = "Cobalt wiki resource budget";
      sliceConfig = {
        MemoryAccounting = true;
        MemoryMax = "2G";
      };
    };
    systemd.services = {
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
          ExecStart = "${packages.deepwell}/bin/deepwell ${runtimeConfig}";
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
          ExecStart = "${packages.framerail}/bin/framerail";
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
