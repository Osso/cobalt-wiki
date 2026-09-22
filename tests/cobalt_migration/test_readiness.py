import json
import threading
import time
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from unittest.mock import patch

from install.nixos.wait_deepwell import wait_for_deepwell


class ReadinessTests(unittest.TestCase):
    def start_server(self, responses, activation_delay=0):
        requests = []

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                requests.append(
                    {
                        "time": time.monotonic(),
                        "request": json.loads(
                            self.rfile.read(int(self.headers["Content-Length"]))
                        ),
                    }
                )
                status, body = responses.pop(0)
                self.send_response(status)
                if status == 503:
                    self.send_header("Retry-After", "1")
                self.end_headers()
                self.wfile.write(json.dumps(body).encode())

            def log_message(self, *args):
                pass

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler, bind_and_activate=False)
        server.server_bind()
        if not activation_delay:
            server.server_activate()

        def serve():
            if activation_delay:
                time.sleep(activation_delay)
                server.server_activate()
            server.serve_forever()

        worker = threading.Thread(target=serve, daemon=True)
        worker.start()
        self.addCleanup(worker.join)
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        return f"http://127.0.0.1:{server.server_port}", requests

    def test_waits_for_successful_ping_after_transient_not_ready(self):
        url, requests = self.start_server(
            [
                (503, {"error": "starting"}),
                (200, {"jsonrpc": "2.0", "id": 1, "result": "pong"}),
            ]
        )
        wait_for_deepwell(url)
        self.assertEqual(len(requests), 2)
        self.assertTrue(
            all(record["request"]["method"] == "ping" for record in requests)
        )
        self.assertGreaterEqual(requests[1]["time"] - requests[0]["time"], 1)

    def test_waits_for_listener_that_is_not_ready_at_process_start(self):
        url, requests = self.start_server(
            [(200, {"jsonrpc": "2.0", "id": 1, "result": "pong"})],
            activation_delay=0.05,
        )
        wait_for_deepwell(url)
        self.assertEqual(len(requests), 1)
        self.assertEqual(requests[0]["request"]["method"], "ping")

    def test_rejects_application_error_in_successful_http_response(self):
        url, requests = self.start_server(
            [(200, {"jsonrpc": "2.0", "id": 1, "error": {"code": -32601}})]
        )
        with self.assertRaises(RuntimeError):
            wait_for_deepwell(url)
        self.assertEqual(len(requests), 1)

    def test_bounds_transient_failure_attempts(self):
        url, requests = self.start_server([(503, {"error": "starting"})] * 6)
        with (
            patch("install.nixos.wait_deepwell.time.sleep"),
            self.assertRaises(RuntimeError),
        ):
            wait_for_deepwell(url)
        self.assertEqual(len(requests), 6)


if __name__ == "__main__":
    unittest.main()
