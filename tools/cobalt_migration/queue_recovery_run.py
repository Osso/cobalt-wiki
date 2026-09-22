"""Apply a sealed candidate plan while its queue consumers/producers are stopped."""

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

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
    with gzip.open(path, "rt") as stream:
        for line in stream:
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
            with gzip.GzipFile(fileobj=raw, mode="wb", compresslevel=1) as writer:
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
                    writer.write(
                        (json.dumps(selected, separators=(",", ":")) + "\n").encode()
                    )
                    writer.flush()
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--valkey-cli", required=True)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument("--sha256", required=True)
    parser.add_argument("--journal", type=Path, required=True)
    parser.add_argument("--cutoff-ms", type=int, required=True)
    parser.add_argument("--seconds", type=int, required=True)
    args = parser.parse_args()
    if not 0 < args.seconds <= 210:
        parser.error("apply budget must be between 1 and 210 seconds")
    with args.plan.open("rb") as stream:
        if hashlib.file_digest(stream, "sha256").hexdigest() != args.sha256:
            parser.error("candidate plan hash does not match")
    os.umask(0o077)
    client = QueueClient(args.valkey_cli, ["-h", "127.0.0.1", "-p", str(args.port)])
    result = apply_plan(client, args.plan, args.journal, args.cutoff_ms, args.seconds)
    print(json.dumps(result))


if __name__ == "__main__":
    main()
