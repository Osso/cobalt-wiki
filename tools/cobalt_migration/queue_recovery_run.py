"""Apply a sealed candidate plan while its queue consumers/producers are stopped."""

import argparse
import gzip
import hashlib
import json
import os
import subprocess
import time
from pathlib import Path

from .queue_recovery import APPLY, RESTORE, SELECT


class QueueClient:
    def __init__(self, executable, connection, keys=("rsmq:job", "rsmq:job:Q")):
        self.command = [executable, *connection, "--json", "-e"]
        self.keys = keys

    def read_depth(self):
        return self.call(["ZCARD", self.keys[0]])

    def read_counters(self):
        return self.call(["HMGET", self.keys[1], "totalsent", "totalrecv"])

    def call(self, arguments, data=None):
        command = self.command + (["-X", "DATA"] if data is not None else [])
        result = subprocess.run(
            command + arguments,
            input=data,
            text=True,
            capture_output=True,
            timeout=15,
            check=True,
        )
        return json.loads(result.stdout)

    def evaluate(self, script, records, cutoff_ms, *, readonly=False):
        return self.call(
            [
                "EVAL_RO" if readonly else "EVAL",
                script,
                "2",
                *self.keys,
                "DATA",
                str(cutoff_ms),
            ],
            json.dumps(records, separators=(",", ":")),
        )


def read_batches(path, size=4096):
    with gzip.open(path, "rt") as stream:
        batch = []
        for line in stream:
            batch.append(json.loads(line))
            if len(batch) == size:
                yield batch
                batch = []
        if batch:
            yield batch


def restore_journal(client, path, cutoff_ms):
    restored = 0
    with path.open("rb") as stream:
        for line in stream:
            # An incomplete final write was never fsynced before APPLY.
            if not line.endswith(b"\n"):
                break
            restored += client.evaluate(RESTORE, json.loads(line), cutoff_ms)
    return restored


def apply_plan(client, plan, journal, cutoff_ms, seconds):
    before = client.read_depth()
    counters = client.read_counters()
    started = time.monotonic()
    result = {"before": before, "removed": 0, "skipped": 0, "batches": 0}
    journal_created = False
    try:
        with journal.open("xb") as raw:
            journal_created = True
            os.chmod(journal, 0o600)
            for candidates in read_batches(plan):
                if time.monotonic() - started > seconds:
                    raise TimeoutError("apply budget reached; restoring journal")
                selected = json.loads(
                    client.evaluate(SELECT, candidates, cutoff_ms, readonly=True)
                )
                result["skipped"] += len(candidates) - len(selected)
                result["batches"] += 1
                if not selected:
                    continue
                raw.write((json.dumps(selected, separators=(",", ":")) + "\n").encode())
                raw.flush()
                os.fsync(raw.fileno())
                removed = client.evaluate(APPLY, selected, cutoff_ms)
                if removed != len(selected):
                    raise RuntimeError("unexpected guarded removal count")
                result["removed"] += removed
            raw.flush()
            os.fsync(raw.fileno())
        result["after"] = client.read_depth()
        if (
            result["after"] != before - result["removed"]
            or client.read_counters() != counters
        ):
            raise RuntimeError("queue changed outside guarded removal")
    except BaseException:
        if journal_created:
            result["restored"] = restore_journal(client, journal, cutoff_ms)
            if client.read_depth() != before or client.read_counters() != counters:
                raise RuntimeError(
                    "rollback queue state does not match initial counters"
                )
        raise
    result["elapsed_seconds"] = round(time.monotonic() - started, 3)
    return result


def require_stopped():
    for unit in ("cobalt-wiki-deepwell", "cobalt-wiki-framerail"):
        result = subprocess.run(
            ["systemctl", "show", unit, "--property=LoadState,ActiveState,MainPID"],
            check=True,
            capture_output=True,
            text=True,
            timeout=10,
        )
        state = dict(
            line.split("=", 1) for line in result.stdout.splitlines() if "=" in line
        )
        if state != {"LoadState": "loaded", "ActiveState": "inactive", "MainPID": "0"}:
            raise RuntimeError(
                f"{unit} must be loaded and stopped before queue recovery"
            )


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("apply", "restore"))
    parser.add_argument("--valkey-cli", required=True)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--plan", type=Path)
    parser.add_argument("--sha256")
    parser.add_argument("--journal", type=Path, required=True)
    parser.add_argument("--cutoff-ms", type=int, required=True)
    parser.add_argument("--seconds", type=int)
    args = parser.parse_args(argv)
    if args.action == "apply":
        if (
            args.plan is None
            or args.sha256 is None
            or args.seconds is None
            or not 0 < args.seconds <= 210
        ):
            parser.error("apply requires a sealed plan and a 1–210 second budget")
        with args.plan.open("rb") as stream:
            if hashlib.file_digest(stream, "sha256").hexdigest() != args.sha256:
                parser.error("candidate plan hash does not match")
    require_stopped()
    os.umask(0o077)
    client = QueueClient(args.valkey_cli, ["-h", "127.0.0.1", "-p", str(args.port)])
    if args.action == "restore":
        result = {
            "restored": restore_journal(client, args.journal, args.cutoff_ms),
            "depth": client.read_depth(),
        }
    else:
        result = apply_plan(
            client, args.plan, args.journal, args.cutoff_ms, args.seconds
        )
    print(json.dumps(result))


if __name__ == "__main__":
    main()
