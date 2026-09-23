"""Synthetic pinned-CDP transport tests; never contact a browser or source site."""

import base64
import hashlib
import json
import subprocess
import threading
import time
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import ClassVar
from urllib.parse import parse_qsl

from tools.cobalt_migration.history_export import HistoryResponse
from tools.cobalt_migration.history_transport import make_history_fetch


ORIGIN = "https://example.test"
TARGET = "A" * 32
VM_PAGE = r"""
const vm = require('node:vm');
let input = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => input += chunk);
process.stdin.on('end', async () => {
  const {expression, timeoutText, moduleStatus} = JSON.parse(input);
  let post;
  const context = vm.createContext({
    location:{origin:'https://example.test'},
    document:{cookie:'wikidot_token7=synthetic-token'},
    AbortSignal,
    jQuery:{param:fields => new URLSearchParams(fields).toString()},
    OZONE:{ajax:{parseResponse:JSON.parse}},
    fetch:async (url, options) => {
      post = {url, method:options.method, redirect:options.redirect,
        credentials:options.credentials, body:options.body};
      return {status:200, type:'basic', headers:{get:() => null},
        text:async () => {
          if (timeoutText) throw Object.assign(new Error('timeout'), {name:'TimeoutError'});
          return JSON.stringify({status:moduleStatus,body:'<div>synthetic revision</div>'});
        }};
    }
  });
  try {
    const result = await vm.runInContext(expression, context);
    console.log(JSON.stringify({result,post}));
  } catch (_) { console.log(JSON.stringify({result:{state:'protocol_error'},post})); }
});
"""
LIST = {
    "moduleName": "history/PageRevisionListModule",
    "page": 1,
    "perpage": 20,
    "page_id": 1310927108,
    "options": {"all": True},
}
SOURCE = {"moduleName": "history/PageSourceModule", "revision_id": 1544524675}


class Runner:
    def __init__(self, result):
        self.result = result
        self.calls = []

    def __call__(self, argv, **kwargs):
        self.calls.append((argv, kwargs))
        if isinstance(self.result, BaseException):
            raise self.result
        if isinstance(self.result, int):
            return subprocess.CompletedProcess(argv, self.result, "", "private stderr")
        return subprocess.CompletedProcess(argv, 0, json.dumps(self.result), "")


class CDPFixture(BaseHTTPRequestHandler):
    target_id = TARGET
    target_origin = ORIGIN
    eval_result: ClassVar[dict] = {
        "state": "done",
        "status": 200,
        "raw": "raw",
        "html": "decoded",
        "retry_after": None,
    }
    requests: ClassVar[list] = []
    execute_js = False
    timeout_text = False
    module_status = "ok"

    def log_message(self, *_args):
        pass

    def do_GET(self):
        if self.path == "/json":
            address = self.server.server_address
            pages = [
                {
                    "id": self.target_id,
                    "type": "page",
                    "url": self.target_origin + "/",
                    "webSocketDebuggerUrl": f"ws://127.0.0.1:{address[1]}/devtools/page/{self.target_id}",
                }
            ]
            body = json.dumps(pages).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if self.path != f"/devtools/page/{self.target_id}":
            self.send_error(404)
            return
        key = self.headers["Sec-WebSocket-Key"]
        accept = base64.b64encode(
            hashlib.sha1(
                (key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()
            ).digest()
        ).decode()
        self.send_response(101, "Switching Protocols")
        self.send_header("Upgrade", "websocket")
        self.send_header("Connection", "Upgrade")
        self.send_header("Sec-WebSocket-Accept", accept)
        self.end_headers()
        header = self.rfile.read(2)
        length = header[1] & 127
        if length == 126:
            length = int.from_bytes(self.rfile.read(2), "big")
        elif length == 127:
            length = int.from_bytes(self.rfile.read(8), "big")
        mask = self.rfile.read(4)
        payload = bytes(
            byte ^ mask[i % 4] for i, byte in enumerate(self.rfile.read(length))
        )
        command = json.loads(payload)
        if self.execute_js:
            executed = subprocess.run(
                ["node", "-e", VM_PAGE],
                input=json.dumps(
                    {
                        "expression": command["params"]["expression"],
                        "timeoutText": self.timeout_text,
                        "moduleStatus": self.module_status,
                    }
                ),
                capture_output=True,
                text=True,
                timeout=2,
                check=True,
            )
            execution = json.loads(executed.stdout)
            self.requests.append(execution["post"])
            result = execution["result"]
        else:
            self.requests.append(command["method"])
            result = self.eval_result
        response = json.dumps(
            {"id": command["id"], "result": {"result": {"value": result}}}
        ).encode()
        if len(response) < 126:
            self.wfile.write(bytes([0x81, len(response)]) + response)
        else:
            self.wfile.write(
                bytes([0x81, 126]) + len(response).to_bytes(2, "big") + response
            )
        self.wfile.flush()
        self.connection.settimeout(1)
        try:
            self.connection.recv(1024)
        except (TimeoutError, ConnectionError, OSError):
            pass


class HistoryTransportTests(unittest.TestCase):
    def factory(self, runner, **options):
        return make_history_fetch(
            ORIGIN,
            TARGET,
            runner=runner,
            node_binary="node",
            timeout_seconds=3,
            **options,
        )

    def test_real_node_completes_pinned_local_cdp_without_waiting_full_deadline(self):
        fixture = type("Fixture", (CDPFixture,), {"requests": []})
        server = ThreadingHTTPServer(("127.0.0.1", 0), fixture)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        url = f"http://127.0.0.1:{server.server_port}"
        fetch = make_history_fetch(ORIGIN, TARGET, cdp_origin=url, timeout_seconds=3)
        started = time.monotonic()
        response = fetch(SOURCE)
        elapsed = time.monotonic() - started
        self.assertEqual(response, HistoryResponse(200, "raw", "decoded"))
        self.assertEqual(fixture.requests, ["Runtime.evaluate"])
        self.assertLess(elapsed, 2)

    def test_real_node_uses_post_token_and_json_options_on_pinned_synthetic_page(self):
        fixture = type("Fixture", (CDPFixture,), {"requests": [], "execute_js": True})
        server = ThreadingHTTPServer(("127.0.0.1", 0), fixture)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        fetch = make_history_fetch(
            ORIGIN,
            TARGET,
            cdp_origin=f"http://127.0.0.1:{server.server_port}",
            timeout_seconds=3,
        )
        response = fetch(LIST)
        self.assertEqual(response.html, "<div>synthetic revision</div>")
        post = fixture.requests[0]
        self.assertEqual(
            (post["url"], post["method"], post["credentials"], post["redirect"]),
            ("/ajax-module-connector.php", "POST", "same-origin", "manual"),
        )
        fields = dict(parse_qsl(post["body"]))
        self.assertEqual(
            fields,
            {
                "moduleName": "history/PageRevisionListModule",
                "page": "1",
                "perpage": "20",
                "page_id": "1310927108",
                "options": '{"all":true}',
                "wikidot_token7": "synthetic-token",
                "callbackIndex": "0",
            },
        )

    def test_module_denial_retains_raw_for_exporter_before_parse_failure(self):
        fixture = type(
            "Fixture",
            (CDPFixture,),
            {
                "requests": [],
                "execute_js": True,
                "module_status": "forbidden",
            },
        )
        server = ThreadingHTTPServer(("127.0.0.1", 0), fixture)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        fetch = make_history_fetch(
            ORIGIN,
            TARGET,
            cdp_origin=f"http://127.0.0.1:{server.server_port}",
            timeout_seconds=3,
        )
        response = fetch(SOURCE)
        self.assertEqual(response.status, 200)
        self.assertIn('"forbidden"', response.raw)
        self.assertEqual(response.html, "")

    def test_text_stream_timeout_is_retryable(self):
        fixture = type(
            "Fixture",
            (CDPFixture,),
            {"requests": [], "execute_js": True, "timeout_text": True},
        )
        server = ThreadingHTTPServer(("127.0.0.1", 0), fixture)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        fetch = make_history_fetch(
            ORIGIN,
            TARGET,
            cdp_origin=f"http://127.0.0.1:{server.server_port}",
            timeout_seconds=3,
        )
        with self.assertRaises(TimeoutError):
            fetch(SOURCE)

    def test_list_returns_raw_and_decoded_html_without_modifying_either(self):
        runner = Runner(
            {
                "state": "done",
                "status": 200,
                "raw": '{"status":"ok","body":"private"}',
                "html": "<table>synthetic history</table>",
                "retry_after": None,
            }
        )
        response = self.factory(runner)(LIST)
        self.assertEqual(
            response,
            HistoryResponse(
                200,
                '{"status":"ok","body":"private"}',
                "<table>synthetic history</table>",
                None,
            ),
        )
        argv, options = runner.calls[0]
        self.assertEqual(argv[:2], ["node", "-e"])
        self.assertEqual(
            json.loads(options["input"]),
            {
                "source_origin": ORIGIN,
                "cdp_origin": "http://127.0.0.1:9222",
                "target_id": TARGET,
                "request": LIST,
                "timeout_ms": 3000,
            },
        )
        self.assertGreater(options["timeout"], 0)

    def test_source_and_permanent_unavailable_response(self):
        runner = Runner(
            {
                "state": "done",
                "status": 404,
                "raw": "denied",
                "html": "",
                "retry_after": None,
            }
        )
        self.assertEqual(
            self.factory(runner)(SOURCE), HistoryResponse(404, "denied", "", None)
        )
        self.assertEqual(json.loads(runner.calls[0][1]["input"])["request"], SOURCE)

    def test_rate_limit_preserves_raw_and_retry_after(self):
        runner = Runner(
            {
                "state": "done",
                "status": 429,
                "raw": "try later",
                "html": "",
                "retry_after": "12",
            }
        )
        self.assertEqual(
            self.factory(runner)(SOURCE), HistoryResponse(429, "try later", "", "12")
        )

    def test_rejects_unlisted_modules_and_malformed_fields_before_network(self):
        runner = Runner(
            {"state": "done", "status": 200, "raw": "", "html": "", "retry_after": None}
        )
        invalid = [
            {**LIST, "moduleName": "history/PageDiffModule"},
            {**LIST, "perpage": 200},
            {**LIST, "page": True},
            {**LIST, "options": {"all": False}},
            {**LIST, "wikidot_token7": "secret"},
            {**SOURCE, "revision_id": -1},
            {**SOURCE, "callbackIndex": 1},
        ]
        for request in invalid:
            with self.subTest(request=request), self.assertRaises(ValueError):
                self.factory(runner)(request)
        with self.assertRaises(TypeError):
            self.factory(runner)(None)
        self.assertEqual(runner.calls, [])

    def test_rejects_invalid_origin_target_and_cdp_endpoint(self):
        runner = Runner(
            {"state": "done", "status": 200, "raw": "", "html": "", "retry_after": None}
        )
        for kwargs in (
            {"source_origin": "https://example.test/path"},
            {"target_id": "not-a-cdp-target"},
            {"cdp_origin": "http://remote.test:9222"},
            {"cdp_origin": "http://127.0.0.1:9222/json"},
        ):
            options = {
                "source_origin": ORIGIN,
                "target_id": TARGET,
                "runner": runner,
                "node_binary": "node",
            }
            options.update(kwargs)
            with self.subTest(kwargs=kwargs), self.assertRaises(ValueError):
                make_history_fetch(**options)
        self.assertEqual(runner.calls, [])

    def test_origin_change_missing_target_redirect_and_module_denial_fail_permanently(
        self,
    ):
        for state in (
            "origin_changed",
            "target_missing",
            "redirect",
            "module_error",
            "protocol_error",
        ):
            with self.subTest(state=state):
                runner = Runner({"state": state})
                with self.assertRaisesRegex(RuntimeError, state):
                    self.factory(runner)(SOURCE)
                self.assertEqual(len(runner.calls), 1)

    def test_timeout_and_connection_failure_are_retryable_without_leaking_stderr(self):
        for state, error in (
            ("timeout", TimeoutError),
            ("network_error", ConnectionError),
        ):
            with self.subTest(state=state), self.assertRaises(error) as raised:
                self.factory(Runner({"state": state}))(SOURCE)
            self.assertNotIn("private", str(raised.exception))
        for failure, expected in (
            (subprocess.TimeoutExpired("node", 3, stderr=b"private"), TimeoutError),
            (OSError("private"), ConnectionError),
        ):
            with self.subTest(failure=failure), self.assertRaises(expected) as raised:
                self.factory(Runner(failure))(SOURCE)
            self.assertNotIn("private", str(raised.exception))

    def test_malformed_protocol_and_failed_node_do_not_leak_private_output(self):
        for result in (
            1,
            {"state": "done", "status": 200, "raw": "private", "html": None},
        ):
            with self.subTest(result=result), self.assertRaises(RuntimeError) as raised:
                self.factory(Runner(result))(SOURCE)
            self.assertNotIn("private", str(raised.exception))


if __name__ == "__main__":
    unittest.main()
