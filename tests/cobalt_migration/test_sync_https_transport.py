"""Socket-level regression for sync uploads to authenticated HTTPS storage."""

import hashlib
import json
import shutil
import ssl
import subprocess
import tempfile
import unittest
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from threading import Thread
from urllib.request import HTTPSHandler, build_opener

from tools.cobalt_migration.poc_import import LoopbackRpc, PocImportError, _NoRedirect
from tools.cobalt_migration.wikidot_sync import upload_blob


@contextmanager
def serving(server):
    thread = Thread(target=server.serve_forever)
    thread.start()
    try:
        yield server
    finally:
        server.shutdown()
        thread.join()
        server.server_close()


def local_certificate(directory):
    """Create a one-day test CA with an IP SAN; OpenSSL is a test prerequisite."""
    openssl = shutil.which("openssl")
    if openssl is None:
        raise RuntimeError("openssl is required for the HTTPS transport test")
    certificate = directory / "localhost.pem"
    key = directory / "localhost.key"
    subprocess.run(
        [
            openssl,
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            str(key),
            "-out",
            str(certificate),
            "-days",
            "1",
            "-subj",
            "/CN=127.0.0.1",
            "-addext",
            "subjectAltName=IP:127.0.0.1",
            "-addext",
            "basicConstraints=critical,CA:TRUE",
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    return certificate, key


class SyncHttpsTransportTests(unittest.TestCase):
    def test_upload_blob_uses_issued_https_url_without_leaking_rpc_session(self):
        payload = b"Wikidot image bytes\x00\xff"
        session = "synthetic-rpc-session"
        received = []
        rpc_requests = []
        signature = "synthetic-signature"
        path = f"/objects/7?signature={signature}"

        class StorageHandler(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def do_PUT(self):
                data = self.rfile.read(int(self.headers["Content-Length"]))
                received.append((self.path, data, dict(self.headers)))
                self.send_response(200)
                self.send_header("Content-Length", "0")
                self.end_headers()

        with tempfile.TemporaryDirectory() as temporary:
            certificate, key = local_certificate(Path(temporary))
            with ThreadingHTTPServer(("127.0.0.1", 0), StorageHandler) as storage:
                tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
                tls.load_cert_chain(certificate, key)
                storage.socket = tls.wrap_socket(storage.socket, server_side=True)
                issued = f"https://127.0.0.1:{storage.server_port}{path}"

                class RpcHandler(BaseHTTPRequestHandler):
                    def log_message(self, *_args):
                        pass

                    def do_POST(self):
                        request = json.loads(
                            self.rfile.read(int(self.headers["Content-Length"]))
                        )
                        rpc_requests.append((self.path, request, dict(self.headers)))
                        reply = {
                            "jsonrpc": "2.0",
                            "id": request["id"],
                            "result": {
                                "pending_blob_id": "pending-7",
                                "presign_url": issued,
                            },
                        }
                        data = json.dumps(reply).encode()
                        self.send_response(200)
                        self.send_header("Content-Length", str(len(data)))
                        self.end_headers()
                        self.wfile.write(data)

                with (
                    serving(storage),
                    ThreadingHTTPServer(("127.0.0.1", 0), RpcHandler) as rpc_server,
                    serving(rpc_server),
                ):
                    client = LoopbackRpc(
                        f"http://127.0.0.1:{rpc_server.server_port}/jsonrpc",
                        session,
                        10,
                    )
                    # Keep the real urllib opener and its no-redirect handler;
                    # trust this certificate only for this client's HTTPS requests.
                    trust = ssl.create_default_context(cafile=str(certificate))
                    client.opener = build_opener(
                        _NoRedirect(), HTTPSHandler(context=trust)
                    )
                    self.assertEqual(upload_blob(client, 2, payload), "pending-7")
                    self.assertEqual(len(rpc_requests), 1)
                    rpc_path, rpc_request, rpc_headers = rpc_requests[0]
                    self.assertEqual(rpc_path, "/jsonrpc")
                    self.assertEqual(rpc_request["method"], "blob_upload")
                    self.assertEqual(
                        rpc_request["params"],
                        {"user_id": 2, "blob_size": len(payload)},
                    )
                    self.assertEqual(rpc_headers["X-Deepwell-Session-Token"], session)
                    self.assertEqual(rpc_headers["X-Deepwell-Site-Id"], "10")
                    self.assertEqual(len(received), 1)
                    storage_path, stored, storage_headers = received[0]
                    self.assertEqual(storage_path, path)
                    self.assertEqual(stored, payload)
                    self.assertEqual(
                        hashlib.sha512(stored).hexdigest(),
                        hashlib.sha512(payload).hexdigest(),
                    )
                    self.assertNotIn("X-Deepwell-Session-Token", storage_headers)
                    self.assertNotIn("X-Deepwell-Site-Id", storage_headers)
                    self.assertNotIn(session, str(storage_headers))
                    with self.assertRaises(PocImportError):
                        client.put(issued + "&changed=1", payload)
                    with self.assertRaises(PocImportError):
                        client.put(issued, payload)
                    self.assertEqual(len(received), 1)
                    self.assertEqual(len(rpc_requests), 1)


if __name__ == "__main__":
    unittest.main()
