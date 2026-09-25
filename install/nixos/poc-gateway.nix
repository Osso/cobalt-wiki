{
  config,
  lib,
  pkgs,
  ...
}:
let
  wiki = config.services.cobaltWiki;
  cfg = wiki.pocGateway;
  hostname = "cobalt-company.sakuin.org";
  nginxConfig = pkgs.writeText "cobalt-poc-nginx.conf" ''
    daemon off;
    worker_processes 1;
    pid /run/cobalt-wiki-gateway/nginx.pid;
    error_log stderr;
    events { worker_connections 256; }
    http {
      access_log off;
      client_body_temp_path /run/cobalt-wiki-gateway/client;
      proxy_temp_path /run/cobalt-wiki-gateway/proxy;
      server {
        listen 127.0.0.1:3088 default_server;
        server_name ${hostname};
        add_header X-Robots-Tag "noindex, nofollow, noarchive" always;
        client_max_body_size 100m;
        proxy_set_header Host ${hostname};
        proxy_set_header X-Forwarded-Host ${hostname};
        proxy_set_header X-Forwarded-Proto https;
        proxy_set_header X-Forwarded-For $remote_addr;
        proxy_set_header X-Wikijump-Site-Id ${toString cfg.siteId};
        proxy_set_header X-Wikijump-Site-Slug cobalt-company;
        proxy_set_header X-Wikijump-Target-Server main;
        proxy_set_header X-Wikijump-User-Id "";
        proxy_set_header Authorization "";

        location ~ ^/(?:robots[.]txt/?$|[.]well-known/?$|-/(?:files|file|download|avatar|code|html|health-check|basic-error)(?:/|$)|local--(?:files|code|html)/|[^/]+/(?:code|html|file|download)/) {
          proxy_pass http://127.0.0.1:3466;
        }
        location = /-/cobalt-theme.css {
          alias /var/lib/cobalt-wiki/poc-theme.css;
          default_type text/css;
        }
        location / {
          proxy_pass http://127.0.0.1:3393;
        }
      }
    }
  '';
in
{
  options.services.cobaltWiki.pocGateway = {
    enable = lib.mkEnableOption "Cobalt POC same-origin gateway";
    siteId = lib.mkOption {
      type = lib.types.ints.positive;
      description = "Actual provisioned cobalt-company site ID; never infer it from seed order.";
    };
  };

  config = lib.mkIf (wiki.enable && cfg.enable) {
    assertions = [
      {
        assertion = wiki.mainDomain == "sakuin.org" && wiki.filesDomain == "sakuin.org";
        message = "The Cobalt POC gateway requires mainDomain and filesDomain sakuin.org for same-origin links.";
      }
    ];
    systemd.services.cobalt-wiki-framerail.environment.ORIGIN = lib.mkForce "https://${hostname}";
    systemd.services.cobalt-wiki-gateway = {
      description = "Cobalt POC same-origin gateway";
      wantedBy = [ "multi-user.target" ];
      requires = [
        "cobalt-wiki-framerail.service"
        "cobalt-wiki-wws.service"
      ];
      after = [
        "cobalt-wiki-framerail.service"
        "cobalt-wiki-wws.service"
      ];
      serviceConfig = {
        User = "cobalt-wiki";
        Group = "cobalt-wiki";
        Slice = "cobalt-wiki.slice";
        RuntimeDirectory = "cobalt-wiki-gateway";
        RuntimeDirectoryMode = "0700";
        UMask = "0077";
        NoNewPrivileges = true;
        PrivateTmp = true;
        ExecStart = "${pkgs.nginx}/bin/nginx -p /run/cobalt-wiki-gateway/ -c ${nginxConfig}";
        Restart = "on-failure";
        RestartSec = "5s";
      };
    };
  };
}
