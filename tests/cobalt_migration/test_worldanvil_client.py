"""Local HTTP integration tests for the create-only World Anvil transport."""

import json
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from unittest.mock import patch
from urllib.parse import parse_qs, urlsplit

from tools.cobalt_migration.worldanvil_client import WorldAnvilClient, WorldAnvilError

WORLD = "fdaa8418-78bd-4c72-9f51-7c837719ea93"
EXISTING = "460c3e07-146f-43ae-8f59-d96f9107a459"
CREATED = "06266035-5630-4b7d-9e45-137bd779381b"


class WorldAnvilClientTests(unittest.TestCase):
    def setUp(self):
        self.articles = [
            {"id": f"00000000-0000-4000-8000-{i:012x}", "title": f"Page {i}"}
            for i in range(1, 76)
        ]
        self.existing = {
            EXISTING: {
                "id": EXISTING,
                "title": "Existing",
                "content": "Keep this unchanged",
            }
        }
        self.requests = []
        self.responses = []
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def do_GET(self):
                self.respond()

            def do_POST(self):
                self.respond()

            def do_PUT(self):
                self.respond()

            def respond(self):
                path = urlsplit(self.path)
                query = parse_qs(path.query)
                length = int(self.headers.get("Content-Length", "0"))
                body = json.loads(self.rfile.read(length)) if length else None
                owner.requests.append(
                    (self.command, path.path, query, body, dict(self.headers))
                )
                if owner.responses:
                    status, payload, headers = owner.responses.pop(0)
                elif self.command == "POST" and path.path.endswith("/world/articles"):
                    start, limit = body["offset"], body["limit"]
                    status, payload, headers = (
                        200,
                        owner.articles[start : start + limit],
                        {},
                    )
                elif self.command == "GET" and path.path.endswith("/article"):
                    status, payload, headers = 200, owner.existing[query["id"][0]], {}
                elif self.command == "GET" and path.path.endswith("/world"):
                    status, payload, headers = (
                        200,
                        {"id": WORLD, "title": "Cobalt Company"},
                        {},
                    )
                elif self.command == "PUT" and path.path.endswith("/article"):
                    owner.existing[CREATED] = {"id": CREATED, **body}
                    status, payload, headers = (
                        200,
                        {"id": CREATED, "title": body["title"]},
                        {},
                    )
                else:
                    status, payload, headers = 404, {"error": "missing"}, {}
                data = (
                    json.dumps(payload).encode()
                    if not isinstance(payload, bytes)
                    else payload
                )
                self.send_response(status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                for name, value in headers.items():
                    self.send_header(name, value)
                self.end_headers()
                self.wfile.write(data)

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.addCleanup(self.server.server_close)
        self.addCleanup(self.server.shutdown)
        self.client = WorldAnvilClient(
            "app-secret",
            "auth-secret",
            "CobaltImporter/1.0 (https://example.org)",
            base_url=f"http://127.0.0.1:{self.server.server_port}/api/external/boromir",
            timeout=2,
        )

    def test_paginates_more_than_fifty_and_reads_full_objects_with_credentials(self):
        self.assertEqual(self.client.list_articles(WORLD), self.articles)
        self.assertEqual(self.client.get_article(EXISTING), self.existing[EXISTING])
        self.assertEqual(self.client.get_world(WORLD)["title"], "Cobalt Company")
        self.assertEqual(
            [(method, path, body) for method, path, _, body, _ in self.requests[:2]],
            [
                (
                    "POST",
                    "/api/external/boromir/world/articles",
                    {"offset": 0, "limit": 50},
                ),
                (
                    "POST",
                    "/api/external/boromir/world/articles",
                    {"offset": 50, "limit": 50},
                ),
            ],
        )
        for method, _, query, _, headers in self.requests:
            headers = {key.lower(): value for key, value in headers.items()}
            self.assertEqual(headers["x-application-key"], "app-secret")
            self.assertEqual(headers["x-auth-token"], "auth-secret")
            self.assertEqual(
                headers["user-agent"], "CobaltImporter/1.0 (https://example.org)"
            )
            self.assertEqual(headers["content-type"], "application/json")
            if method == "GET":
                self.assertEqual(query["granularity"], ["2"])

    def test_create_preserves_existing_article_and_never_addresses_existing_id(self):
        before = self.existing[EXISTING].copy()
        result = self.client.create_article(
            WORLD,
            {"title": "Imported", "templateType": "article", "content": "New body"},
        )
        self.assertEqual(result, {"id": CREATED, "title": "Imported"})
        self.assertEqual(self.existing[EXISTING], before)
        self.assertEqual(self.existing[CREATED]["world"], {"id": WORLD})
        self.assertEqual(
            [request[0:3] for request in self.requests],
            [("PUT", "/api/external/boromir/article", {})],
        )

    def test_uncertain_creation_response_is_not_retried(self):
        self.responses.append(
            (503, {"error": "possibly created", "secret": "auth-secret"}, {})
        )
        with self.assertRaises(WorldAnvilError) as caught:
            self.client.create_article(
                WORLD, {"title": "Imported", "templateType": "article"}
            )
        self.assertEqual(len(self.requests), 1)
        self.assertNotIn("auth-secret", str(caught.exception))
        self.assertNotIn("app-secret", str(caught.exception))

    def test_read_retries_transient_status_respecting_retry_after(self):
        self.responses.extend(
            [
                (429, {}, {"Retry-After": "1"}),
                (503, {}, {}),
                (200, {"id": WORLD, "title": "Cobalt Company"}, {}),
            ]
        )
        with patch("tools.cobalt_migration.worldanvil_client.time.sleep") as sleep:
            self.assertEqual(self.client.get_world(WORLD)["id"], WORLD)
        self.assertEqual(len(self.requests), 3)
        self.assertGreaterEqual(sleep.call_args_list[0].args[0], 1)
        self.assertEqual(sleep.call_count, 2)

    def test_exact_full_page_requires_empty_terminal_page(self):
        self.articles = self.articles[:50]
        self.assertEqual(len(self.client.list_articles(WORLD)), 50)
        self.assertEqual([request[3]["offset"] for request in self.requests], [0, 50])

    def test_invalid_pagination_rejects_duplicates(self):
        duplicate = [{"id": self.articles[0]["id"], "title": "Duplicate"}]
        self.responses.extend([(200, self.articles[:50], {}), (200, duplicate, {})])
        with self.assertRaises(WorldAnvilError):
            self.client.list_articles(WORLD)
        self.assertEqual(len(self.requests), 2)

    def test_rejects_bad_article_page(self):
        self.responses.append((200, [{"id": "bad", "title": "Invalid"}], {}))
        with self.assertRaises(WorldAnvilError):
            self.client.list_articles(WORLD)
        self.assertEqual(len(self.requests), 1)

    def test_rejects_malformed_and_failed_responses_without_secrets(self):
        for response in [
            (200, b"not json", {}),
            (200, [], {}),
            (401, {"token": "auth-secret"}, {}),
        ]:
            with self.subTest(response=response[0]):
                self.responses.append(response)
                with self.assertRaises(WorldAnvilError) as caught:
                    self.client.get_world(WORLD)
                self.assertNotIn("auth-secret", str(caught.exception))

    def test_rejects_invalid_article_and_create_input_before_network(self):
        self.responses.append((200, {"id": "not-a-uuid", "title": "Wrong"}, {}))
        with self.assertRaises(WorldAnvilError):
            self.client.get_article(EXISTING)
        with self.assertRaises(ValueError):
            self.client.create_article(WORLD, {"title": "", "templateType": "article"})
        with self.assertRaises(ValueError):
            self.client.create_article(
                WORLD, {"title": "New", "templateType": "article", "id": EXISTING}
            )
        self.assertEqual(len(self.requests), 1)


if __name__ == "__main__":
    unittest.main()
