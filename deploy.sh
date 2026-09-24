#!/usr/bin/env python3
"""Build and restart the existing local preview Deepwell service only."""

from pathlib import Path
import subprocess


root = Path(__file__).resolve().parent
unit = "cobalt-local-full-deepwell"

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
subprocess.run(["systemctl", "--user", "restart", unit], check=True)
