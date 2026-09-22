import json
import unittest

from tools.cobalt_migration.queue_duplicates import duplicate_candidates


SITE_ID = 6000000
CUTOFF_MS = 1790110000000


def payload(
    *, site_id=SITE_ID, category_id=100000002, page_id=3000000104, depth=2, kind="nav"
):
    return json.dumps(
        {
            "job": "rerender_page",
            "data": {
                "id": {
                    "site_id": site_id,
                    "category_id": category_id,
                    "page_id": page_id,
                },
                "depth": depth,
                "type": kind,
            },
        },
        separators=(",", ":"),
    )


def record(message_id, raw_payload, *, score=CUTOFF_MS, rc=None, fr=None):
    return {
        "id": message_id,
        "score": str(score),
        "payload": raw_payload,
        "rc": rc,
        "fr": fr,
    }


class DuplicateCandidatesTest(unittest.TestCase):
    def candidates(self, records):
        return [
            item["record"]["id"]
            for item in duplicate_candidates(iter(records), SITE_ID, CUTOFF_MS)
        ]

    def test_candidate_retains_original_record_and_keeper_for_live_revalidation(self):
        raw = payload()
        later = record("later", raw)
        self.assertEqual(
            list(
                duplicate_candidates([record("keeper", raw), later], SITE_ID, CUTOFF_MS)
            ),
            [{"keeper_id": "keeper", "record": later}],
        )

    def test_same_bytes_keep_first_eligible_id_and_offer_only_later_ids(self):
        raw = payload()
        self.assertEqual(
            self.candidates([record("a", raw), record("b", raw), record("c", raw)]),
            ["b", "c"],
        )

    def test_different_bytes_and_meaningful_job_fields_are_not_conflated(self):
        raw = payload()
        differently_serialized = json.dumps(json.loads(raw), sort_keys=True)
        records = [
            record("keeper", raw),
            record("other-bytes", differently_serialized),
            record("other-depth", payload(depth=1)),
            record("other-category", payload(category_id=100000003)),
            record("other-site", payload(site_id=SITE_ID + 1)),
            record("other-type", payload(kind="full")),
            record("repeat", raw),
        ]
        self.assertEqual(self.candidates(records), ["repeat"])

    def test_unknown_or_malformed_payloads_are_not_candidates_or_keepers(self):
        valid = json.loads(payload())
        cases = [
            "not json",
            "[]",
            json.dumps({"job": "prune_sessions", "data": valid["data"]}),
            json.dumps({**valid, "extra": True}),
            json.dumps({**valid, "data": {**valid["data"], "extra": True}}),
            json.dumps(
                {
                    **valid,
                    "data": {
                        **valid["data"],
                        "id": {**valid["data"]["id"], "extra": True},
                    },
                }
            ),
            payload(site_id=True),
            payload(category_id=True),
            payload(page_id=0),
            payload(depth=False),
            payload(depth=-1),
            payload(kind="NavigationOnly"),
            payload().replace(
                '"job":"rerender_page"', '"job":"unknown","job":"rerender_page"'
            ),
        ]
        records = [
            entry
            for n, raw in enumerate(cases)
            for entry in (record(f"bad-{n}", raw), record(f"bad-repeat-{n}", raw))
        ]
        records.extend([record("keeper", payload()), record("candidate", payload())])
        self.assertEqual(self.candidates(records), ["candidate"])

    def test_future_received_and_invalid_score_jobs_are_preserved(self):
        raw = payload()
        records = [
            record("future", raw, score=CUTOFF_MS + 1),
            record("received", raw, rc="1"),
            record("first-received", raw, fr="1790100000000"),
            record("invalid-score", raw, score="not-a-timestamp"),
            record("keeper", raw),
            record("candidate", raw),
        ]
        self.assertEqual(self.candidates(records), ["candidate"])

    def test_repeated_scan_ids_never_mark_keeper_for_removal(self):
        raw = payload()
        self.assertEqual(
            self.candidates(
                [
                    record("keeper", raw),
                    record("keeper", raw),
                    record("candidate", raw),
                    record("keeper", raw),
                    record("candidate", raw),
                ]
            ),
            ["candidate", "candidate"],
        )


if __name__ == "__main__":
    unittest.main()
