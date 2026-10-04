"""Exercise the production deploy CLI with isolated external command fixtures."""

import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

COMMAND_FIXTURE = """#!/usr/bin/env python3
import hashlib
import json
import os
from pathlib import Path
import sys

name = Path(sys.argv[0]).name
args = sys.argv[1:]
entry = {"command": name, "args": args}
if name == "ssh" and ("tee " in " ".join(args) or "cat >" in " ".join(args)):
    entry["stdin"] = sys.stdin.read()
with open(os.environ["COMMAND_LOG"], "a") as log:
    log.write(json.dumps(entry) + "\\n")
if name == "nix":
    if args[0] == os.environ.get("FAIL_NIX"):
        sys.exit(75)
    if args[0] == "build":
        print("/nix/store/fixture-wws")
if name == "ssh" and ".lock-sha256" in " ".join(args):
    root = Path(os.environ["FIXTURE_ROOT"])
    print(hashlib.sha256((root / "framerail/pnpm-lock.yaml").read_bytes()).hexdigest())
if name == "git" and "rev-parse" in args:
    print("feed12345678")
"""


class DevelopmentDeployTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="cobalt-deploy-test-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        source = Path(__file__).resolve().parents[2] / "install/dev-deploy.sh"
        script = self.root / "install/dev-deploy.sh"
        script.parent.mkdir()
        shutil.copy2(source, script)
        (self.root / "framerail/node_modules").mkdir(parents=True)
        (self.root / "framerail/build").mkdir()
        (self.root / "framerail/pnpm-lock.yaml").write_text("fixture lockfile\n")
        self.log = self.root / "commands.jsonl"
        commands = self.root / "commands"
        commands.mkdir()
        for name in ("nix", "ssh", "rsync", "git"):
            program = commands / name
            program.write_text(COMMAND_FIXTURE)
            program.chmod(0o755)
        self.environment = {
            **os.environ,
            "PATH": f"{commands}:{os.environ['PATH']}",
            "COBALT_HOST": "fixture-host",
            "COMMAND_LOG": str(self.log),
            "FIXTURE_ROOT": str(self.root),
        }

    def deploy(self, target, fail=None):
        environment = {**self.environment}
        if fail:
            environment["FAIL_NIX"] = fail
        result = subprocess.run(
            [str(self.root / "install/dev-deploy.sh"), target],
            env=environment,
            text=True,
            capture_output=True,
            timeout=30,
            check=False,
        )
        events = []
        if self.log.exists():
            events = [json.loads(line) for line in self.log.read_text().splitlines()]
        return result, events

    def assert_wws_release(self, events):
        copied = [
            event
            for event in events
            if event["command"] == "nix" and event["args"][0] == "copy"
        ]
        self.assertEqual(len(copied), 1)
        self.assertEqual(
            copied[0]["args"],
            ["copy", "--to", "ssh://fixture-host", "/nix/store/fixture-wws"],
        )
        overrides = [event for event in events if "stdin" in event]
        self.assertEqual(len(overrides), 1)
        self.assertIn(
            "cobalt-wiki-wws.service.d/release.conf", " ".join(overrides[0]["args"])
        )
        self.assertEqual(
            overrides[0]["stdin"].splitlines(),
            ["[Service]", "ExecStart=", "ExecStart=/nix/store/fixture-wws/bin/wws"],
        )
        remote_commands = "\n".join(
            " ".join(event["args"]) for event in events if event["command"] == "ssh"
        )
        self.assertIn("systemctl daemon-reload", remote_commands)
        self.assertIn("systemctl restart cobalt-wiki-wws", remote_commands)
        self.assertIn("cobalt-wiki-gateway", remote_commands)
        self.assertNotIn("nixos-rebuild", remote_commands)
        self.assertNotIn("S3_", overrides[0]["stdin"])

    def test_wws_deploy_copies_package_and_activates_only_release_executable(self):
        result, events = self.deploy("wws")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_wws_release(events)
        self.assertFalse(any(event["command"] == "rsync" for event in events))

    def test_all_includes_wws_and_builds_it_before_service_mutations(self):
        result, events = self.deploy("all")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_wws_release(events)
        first_ssh = next(
            index for index, event in enumerate(events) if event["command"] == "ssh"
        )
        copy_index = next(
            index
            for index, event in enumerate(events)
            if event["command"] == "nix" and event["args"][0] == "copy"
        )
        self.assertLess(copy_index, first_ssh)
        uploads = "\n".join(
            " ".join(event["args"]) for event in events if event["command"] == "rsync"
        )
        self.assertIn("/deepwell/deepwell.new", uploads)
        self.assertIn("/framerail/build/", uploads)

    def test_failed_wws_build_does_not_copy_or_mutate_services(self):
        result, events = self.deploy("wws", fail="build")
        self.assertEqual(result.returncode, 75)
        self.assertEqual([event["command"] for event in events], ["nix"])

    def test_failed_closure_copy_does_not_mutate_services(self):
        result, events = self.deploy("wws", fail="copy")
        self.assertEqual(result.returncode, 75)
        self.assertEqual([event["command"] for event in events], ["nix", "nix"])


if __name__ == "__main__":
    unittest.main()
