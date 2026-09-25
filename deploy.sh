#!/usr/bin/env python3
"""Build, migrate, and restart the existing local preview Deepwell service only."""

import json
import os
import shutil
import subprocess
from pathlib import Path
from urllib.parse import urlsplit


def local_database_url():
    config = Path.home() / ".local/share/cobalt-wiki/local-full/environment.json"
    try:
        url = json.loads(config.read_text())["DATABASE_URL"]
        database = urlsplit(url)
        valid = (
            database.scheme in ("postgres", "postgresql")
            and database.hostname in ("127.0.0.1", "localhost")
            and database.port == 25432
            and database.path == "/cobalt_local_full"
            and not database.query
            and not database.fragment
        )
    except (OSError, ValueError, KeyError, TypeError, AttributeError):
        valid = False
    if not valid:
        raise SystemExit(
            "Local preview DATABASE_URL must target loopback:25432/cobalt_local_full"
        )
    return url


root = Path(__file__).resolve().parent
unit = "cobalt-local-full-deepwell"
database_url = local_database_url()
if not shutil.which("sqlx"):
    raise SystemExit(
        "sqlx not on PATH; run via nix shell nixpkgs#sqlx-cli --command ./deploy.sh"
    )

try:
    subprocess.run(["systemctl", "--user", "is-active", "--quiet", unit], check=True)
except subprocess.CalledProcessError as error:
    raise SystemExit(
        f"Local preview unit {unit} is not active; refusing to build or restart"
    ) from error

subprocess.run(
    ["cargo", "build", "--offline", "--locked", "--bin", "deepwell"],
    cwd=root / "deepwell",
    check=True,
)
migration_env = os.environ.copy()
migration_env["DATABASE_URL"] = database_url
migration = subprocess.run(
    ["sqlx", "migrate", "run"],
    cwd=root / "deepwell",
    env=migration_env,
    capture_output=True,
    check=False,
)
migration_log = (
    Path.home() / ".local/share/cobalt-wiki/local-full/deploy-migrations.log"
)
with os.fdopen(
    os.open(migration_log, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600), "wb"
) as log:
    os.fchmod(log.fileno(), 0o600)
    log.write(migration.stdout + migration.stderr)
if migration.returncode:
    raise SystemExit(
        f"SQLx migrations failed; see {migration_log}; refusing to restart local preview unit"
    )
subprocess.run(["systemctl", "--user", "restart", unit], check=True)
