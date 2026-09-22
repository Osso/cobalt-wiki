"""Explicit local Redis protocol tests; only UUID-namespaced fixture keys change."""

import json
import os
import subprocess
import tempfile
from pathlib import Path
import gzip
import unittest
import uuid

from tools.cobalt_migration.queue_recovery import APPLY, RESTORE, SELECT
from tools.cobalt_migration.queue_recovery_run import (
    QueueClient,
    apply_plan,
    restore_journal,
)


SOCKET = os.environ.get("QUEUE_TEST_SOCKET")
CUTOFF = 1790117529000


def record(name, kind="nav", *, score=CUTOFF, rc=None, site=6000000):
    payload = json.dumps(
        {
            "job": "rerender_page",
            "data": {
                "id": {
                    "site_id": site,
                    "category_id": 100000002,
                    "page_id": 3000000104,
                },
                "depth": 2,
                "type": kind,
            },
        },
        separators=(",", ":"),
    )
    return {"id": name, "score": str(score), "payload": payload, "rc": rc, "fr": None}


@unittest.skipUnless(SOCKET, "set QUEUE_TEST_SOCKET to an isolated local Redis socket")
class QueueRecoveryTest(unittest.TestCase):
    def setUp(self):
        self.queue = "queue-recovery-test:" + uuid.uuid4().hex
        self.hash = self.queue + ":Q"

    def command(self, *args, input_data=None, succeeds=True):
        command = ["valkey-cli", "-s", SOCKET, "--json", "-e"]
        if input_data is not None:
            command += ["-X", "DATA"]
        result = subprocess.run(
            command + list(args),
            input=input_data,
            text=True,
            capture_output=True,
            timeout=10,
        )
        if not succeeds:
            self.assertNotEqual(result.returncode, 0)
            return
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        return json.loads(result.stdout)

    def evaluate(self, script, candidates, *, succeeds=True):
        return self.command(
            "EVAL",
            script,
            "2",
            self.queue,
            self.hash,
            "DATA",
            str(CUTOFF),
            input_data=json.dumps(candidates),
            succeeds=succeeds,
        )

    def seed(self, rows):
        self.evaluate(
            """
        for _,r in ipairs(cjson.decode(ARGV[1])) do
          redis.call('ZADD',KEYS[1],r.score,r.id)
          redis.call('HSET',KEYS[2],r.id,r.payload)
          if r.rc~=cjson.null then redis.call('HSET',KEYS[2],r.id..':rc',r.rc) end
        end
        redis.call('HSET',KEYS[2],'totalsent','123','totalrecv','7')
        return #cjson.decode(ARGV[1])
        """,
            rows,
        )

    def tearDown(self):
        self.command("DEL", self.queue, self.hash)

    def select(self, candidates):
        result = json.loads(self.evaluate(SELECT, candidates))
        return result if result else []

    def write_plan(self, path, rows):
        with gzip.open(path, "wt") as stream:
            for row in rows[1:]:
                stream.write(
                    json.dumps({"keeper_id": rows[0]["id"], "record": row}) + "\n"
                )

    def test_driver_journals_multiple_batches_and_can_restore_them(self):
        rows = [record("keeper")] + [record(f"driver-{i}") for i in range(5000)]
        self.seed(rows)
        client = QueueClient("valkey-cli", ["-s", SOCKET], (self.queue, self.hash))
        with tempfile.TemporaryDirectory() as directory:
            plan, journal = Path(directory) / "plan.gz", Path(directory) / "journal.gz"
            self.write_plan(plan, rows)
            result = apply_plan(client, plan, journal, CUTOFF, 10)
            self.assertEqual(result["removed"], 5000)
            self.assertGreater(result["batches"], 1)
            self.assertEqual(client.read_depth(), 1)
            self.assertEqual(restore_journal(client, journal, CUTOFF), 5000)
            self.assertEqual(client.read_depth(), 5001)

    def test_driver_restores_after_ambiguous_completed_write(self):
        class LostReply(QueueClient):
            def evaluate(self, script, records, cutoff_ms, *, readonly=False):
                result = super().evaluate(script, records, cutoff_ms, readonly=readonly)
                if script == APPLY:
                    raise ConnectionError("reply lost after completed mutation")
                return result

        rows = [record("keeper"), record("duplicate")]
        self.seed(rows)
        client = LostReply("valkey-cli", ["-s", SOCKET], (self.queue, self.hash))
        with tempfile.TemporaryDirectory() as directory:
            plan, journal = Path(directory) / "plan.gz", Path(directory) / "journal.gz"
            self.write_plan(plan, rows)
            with self.assertRaises(ConnectionError):
                apply_plan(client, plan, journal, CUTOFF, 10)
            self.assertEqual(client.read_depth(), 2)
            self.assertEqual(
                self.command("HGET", self.hash, "duplicate"), rows[1]["payload"]
            )

    def test_existing_journal_is_not_replayed_or_overwritten(self):
        rows = [record("keeper"), record("duplicate")]
        self.seed(rows)
        client = QueueClient("valkey-cli", ["-s", SOCKET], (self.queue, self.hash))
        with tempfile.TemporaryDirectory() as directory:
            plan, journal = Path(directory) / "plan.gz", Path(directory) / "journal.gz"
            self.write_plan(plan, rows)
            journal.write_bytes(b"existing unrelated file")
            with self.assertRaises(FileExistsError):
                apply_plan(client, plan, journal, CUTOFF, 10)
            self.assertEqual(journal.read_bytes(), b"existing unrelated file")
            self.assertEqual(client.read_depth(), 2)

    def test_preserves_non_navigation_received_future_and_foreign_work(self):
        rows = [
            record("keeper"),
            record("duplicate"),
            record("received", rc="1"),
            record("future", score=CUTOFF + 1),
            record("full-keeper", "full"),
            record("full-duplicate", "full"),
            record("foreign-keeper", site=6000001),
            record("foreign-duplicate", site=6000001),
        ]
        self.seed(rows)
        requested = [{"keeper_id": "keeper", "record": row} for row in rows[1:4]] + [
            {"keeper_id": "full-keeper", "record": rows[5]},
            {"keeper_id": "foreign-keeper", "record": rows[7]},
        ]
        selected = self.select(requested)
        self.assertEqual(selected, [{"keeper_id": "keeper", "record": rows[1]}])
        self.assertEqual(self.evaluate(APPLY, selected), 1)
        self.assertEqual(self.command("ZCARD", self.queue), len(rows) - 1)
        self.assertEqual(
            self.command("HMGET", self.hash, "totalsent", "totalrecv"), ["123", "7"]
        )

    def test_changed_batch_aborts_before_any_deletion(self):
        rows = [record("keeper"), record("first"), record("second")]
        self.seed(rows)
        selected = self.select(
            [{"keeper_id": "keeper", "record": row} for row in rows[1:]]
        )
        self.command("HSET", self.hash, "second", "changed")
        self.evaluate(APPLY, selected, succeeds=False)
        self.assertEqual(self.command("ZCARD", self.queue), 3)
        self.assertEqual(self.command("HGET", self.hash, "first"), rows[1]["payload"])

    def test_lost_or_received_keeper_is_not_replaced_by_assumption(self):
        rows = [record("keeper"), record("duplicate")]
        self.seed(rows)
        requested = [{"keeper_id": "keeper", "record": rows[1]}]
        self.command("HSET", self.hash, "keeper:rc", "1")
        self.assertEqual(self.select(requested), [])
        self.command("ZREM", self.queue, "keeper")
        self.assertEqual(self.select(requested), [])

    def test_restore_is_idempotent_and_rejects_conflicting_live_state(self):
        rows = [record("keeper"), record("duplicate")]
        self.seed(rows)
        selected = self.select([{"keeper_id": "keeper", "record": rows[1]}])
        self.assertEqual(self.evaluate(APPLY, selected), 1)
        self.assertEqual(self.evaluate(RESTORE, selected), 1)
        self.assertEqual(self.evaluate(RESTORE, selected), 0)
        self.assertEqual(
            self.command("HGET", self.hash, "duplicate"), rows[1]["payload"]
        )
        self.assertEqual(
            self.command("ZSCORE", self.queue, "duplicate"), int(rows[1]["score"])
        )
        self.command("HSET", self.hash, "duplicate", "conflicting")
        self.evaluate(RESTORE, selected, succeeds=False)
        self.assertEqual(self.command("HGET", self.hash, "duplicate"), "conflicting")

    def test_multiple_batches_keep_one_copy_and_restore_every_record(self):
        rows = [record("keeper")] + [
            record(f"duplicate-{index}") for index in range(5000)
        ]
        self.seed(rows)
        batches = []
        for start in range(1, len(rows), 2048):
            selected = self.select(
                [
                    {"keeper_id": "keeper", "record": row}
                    for row in rows[start : start + 2048]
                ]
            )
            self.assertEqual(self.evaluate(APPLY, selected), len(selected))
            batches.append(selected)
        self.assertGreater(len(batches), 1)
        self.assertEqual(self.command("ZCARD", self.queue), 1)
        for batch in batches:
            self.evaluate(RESTORE, batch)
        self.assertEqual(self.command("ZCARD", self.queue), len(rows))
        self.assertEqual(self.command("HGET", self.hash, "keeper"), rows[0]["payload"])
