"""Exercise the local deploy entry point without calling real service or DB tools."""

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
DEPLOY = ROOT / "deploy.sh"
URL = "postgres://preview:private-password@127.0.0.1:25432/cobalt_local_full"
FAKE_TOOL = """#!{python}
import hashlib
import json
import os
import sys
from pathlib import Path

record = {{
    "tool": Path(sys.argv[0]).name,
    "args": sys.argv[1:],
    "cwd": os.getcwd(),
    "database_url_digest": hashlib.sha256(os.environ.get("DATABASE_URL", "").encode()).hexdigest(),
}}
with open(os.environ["CALL_LOG"], "a", encoding="utf-8") as log:
    log.write(json.dumps(record) + "\\n")
if os.environ.get("FAIL_TOOL") == record["tool"]:
    print("synthetic tool failure", file=sys.stderr)
    sys.exit(9)
"""


class LocalDeployTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.home = Path(self.temporary.name)
        self.bin = self.home / "bin"
        self.bin.mkdir()
        self.log = self.home / "calls.jsonl"
        for tool in ("systemctl", "cargo", "sqlx"):
            script = self.bin / tool
            script.write_text(FAKE_TOOL.format(python=sys.executable))
            script.chmod(0o755)
        self.set_url(URL)

    def set_url(self, url):
        config = self.home / ".local/share/cobalt-wiki/local-full/environment.json"
        config.parent.mkdir(parents=True, exist_ok=True)
        config.write_text(json.dumps({"DATABASE_URL": url}))

    def deploy(self, *, fail_tool=None, missing_tool=None):
        if missing_tool:
            (self.bin / missing_tool).unlink()
        env = {
            "HOME": str(self.home),
            "PATH": str(self.bin),
            "CALL_LOG": str(self.log),
        }
        if fail_tool:
            env["FAIL_TOOL"] = fail_tool
        return subprocess.run(
            [sys.executable, str(DEPLOY)],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )

    def calls(self):
        if not self.log.exists():
            return []
        return [json.loads(line) for line in self.log.read_text().splitlines()]

    def test_wrong_database_aborts_without_running_tools_or_disclosing_url(self):
        self.set_url("postgres://preview:private-password@localhost:25432/other")
        result = self.deploy()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.calls(), [])
        self.assertNotIn("private-password", result.stderr)
        self.assertNotIn("private-password", result.stdout)

    def test_other_host_or_port_aborts_without_running_tools(self):
        for url in (
            "postgres://preview:private-password@remote.example:25432/cobalt_local_full",
            "postgres://preview:private-password@localhost:5432/cobalt_local_full",
            "postgres://preview:private-password@localhost:25432/cobalt_local_full?host=remote",
        ):
            with self.subTest(url=url):
                self.set_url(url)
                result = self.deploy()
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.calls(), [])
                self.assertNotIn("private-password", result.stderr)

    def test_inactive_unit_aborts_before_build(self):
        result = self.deploy(fail_tool="systemctl")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual([call["tool"] for call in self.calls()], ["systemctl"])

    def test_missing_sqlx_aborts_before_build(self):
        result = self.deploy(missing_tool="sqlx")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.calls(), [])
        self.assertIn("sqlx", result.stderr)

    def test_compile_failure_does_not_migrate_or_restart(self):
        result = self.deploy(fail_tool="cargo")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            [call["tool"] for call in self.calls()], ["systemctl", "cargo"]
        )

    def test_migration_failure_does_not_restart(self):
        result = self.deploy(fail_tool="sqlx")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            [call["tool"] for call in self.calls()], ["systemctl", "cargo", "sqlx"]
        )

    def test_migration_failure_retains_private_diagnostics(self):
        result = self.deploy(fail_tool="sqlx")
        self.assertNotEqual(result.returncode, 0)
        diagnostics = (
            self.home / ".local/share/cobalt-wiki/local-full/deploy-migrations.log"
        )
        self.assertTrue(
            diagnostics.exists(), "migration failure diagnostics must be retained"
        )
        self.assertIn("synthetic tool failure", diagnostics.read_text())
        self.assertEqual(diagnostics.stat().st_mode & 0o777, 0o600)
        self.assertIn(str(diagnostics), result.stderr)

    def test_success_builds_then_migrates_then_restarts_with_local_database(self):
        result = self.deploy()
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.calls()
        self.assertEqual(
            [(call["tool"], call["args"]) for call in calls],
            [
                (
                    "systemctl",
                    ["--user", "is-active", "--quiet", "cobalt-local-full-deepwell"],
                ),
                ("cargo", ["build", "--offline", "--locked", "--bin", "deepwell"]),
                ("sqlx", ["migrate", "run"]),
                ("systemctl", ["--user", "restart", "cobalt-local-full-deepwell"]),
            ],
        )
        self.assertEqual(calls[1]["cwd"], str(ROOT / "deepwell"))
        self.assertEqual(calls[2]["cwd"], str(ROOT / "deepwell"))
        self.assertEqual(
            calls[2]["database_url_digest"], hashlib.sha256(URL.encode()).hexdigest()
        )
        self.assertNotIn(
            "private-password", result.stdout + result.stderr + self.log.read_text()
        )


if __name__ == "__main__":
    unittest.main()
