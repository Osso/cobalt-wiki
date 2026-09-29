"""Exercise the generated production gateway with local held image responses.

Run with NGINX_BIN pointing to the deployed Nginx build. No production services,
ports, files or requests are used; Nix only evaluates the checked-out module.
"""

import asyncio
import contextlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import unittest


REPO = Path(__file__).resolve().parents[2]
IMAGE_REQUESTS = 350
IDLE_CONNECTIONS = 64


def generated_gateway_config():
    expression = """
      let
        gateway = import MODULE {
          lib.mkIf = condition: content: assert condition; { inherit content; };
          pkgs = { nginx = "unused"; writeText = name: text: text; };
          config.services.cobaltWiki = {
            enable = true; mainDomain = "sakuin.org"; filesDomain = "sakuin.org";
            pocGateway = { enable = true; siteId = 6000000; };
          };
        };
      in gateway.config.content.systemd.services.cobalt-wiki-gateway.serviceConfig.ExecStart
    """.replace("MODULE", json.dumps(str(REPO / "install/nixos/poc-gateway.nix")))
    result = subprocess.run(
        ["nix", "eval", "--impure", "--raw", "--expr", expression],
        capture_output=True,
        text=True,
        check=True,
        cwd=REPO,
        timeout=30,
    )
    return result.stdout.split(" -c ", 1)[1]


def unused_port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


async def read_response(reader):
    head = await reader.readuntil(b"\r\n\r\n")
    lines = head.decode("latin1").split("\r\n")
    status = int(lines[0].split()[1])
    headers = dict(line.lower().split(":", 1) for line in lines[1:] if ":" in line)
    body = await reader.readexactly(int(headers["content-length"].strip()))
    return status, body


class GatewayConnectionCapacity(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.nginx_binary = os.environ["NGINX_BIN"]
        config = await asyncio.to_thread(generated_gateway_config)
        self.temp = tempfile.TemporaryDirectory(prefix="cobalt-gateway-connections-")
        self.root = Path(self.temp.name)
        self.release = asyncio.Event()
        self.all_arrived = asyncio.Event()
        self.arrived = 0
        self.active = 0
        self.peak = 0
        self.headers = []
        self.backend_tasks = set()
        self.idle = []
        self.expected_images = 0
        self.upstream = None
        self.nginx = None
        self.log = None
        self.addAsyncCleanup(self.cleanup_fixture)
        self.upstream = await asyncio.start_server(
            self.handle_upstream, "127.0.0.1", 0, backlog=1024
        )
        upstream_port = self.upstream.sockets[0].getsockname()[1]
        self.gateway_port = unused_port()
        config = config.replace("/run/cobalt-wiki-gateway", str(self.root))
        # Reveal recycling warnings that production's error-level logging omits.
        config = config.replace("error_log stderr;", "error_log stderr warn;")
        config = config.replace("127.0.0.1:3088", f"127.0.0.1:{self.gateway_port}")
        config = config.replace("127.0.0.1:3466", f"127.0.0.1:{upstream_port}")
        config = config.replace("127.0.0.1:3393", f"127.0.0.1:{upstream_port}")
        (self.root / "nginx.conf").write_text(config)
        self.log = (self.root / "nginx.log").open("wb")
        self.nginx = subprocess.Popen(
            [
                self.nginx_binary,
                "-e",
                "stderr",
                "-p",
                str(self.root) + "/",
                "-c",
                str(self.root / "nginx.conf"),
            ],
            stdout=self.log,
            stderr=self.log,
            start_new_session=True,
        )
        for _ in range(100):
            if self.nginx.poll() is not None:
                raise RuntimeError((self.root / "nginx.log").read_text())
            try:
                _, writer = await asyncio.open_connection(
                    "127.0.0.1", self.gateway_port
                )
                writer.close()
                await writer.wait_closed()
                return
            except ConnectionRefusedError:
                await asyncio.sleep(0.02)
        raise RuntimeError("isolated gateway did not start")

    async def handle_upstream(self, reader, writer):
        task = asyncio.current_task()
        self.backend_tasks.add(task)
        try:
            head = await reader.readuntil(b"\r\n\r\n")
            lines = head.decode("latin1").split("\r\n")
            path = lines[0].split()[1]
            self.headers.append(
                dict(line.lower().split(":", 1) for line in lines[1:] if ":" in line)
            )
            if "/image-" in path:
                self.arrived += 1
                self.active += 1
                self.peak = max(self.peak, self.active)
                if self.arrived == self.expected_images:
                    self.all_arrived.set()
                await self.release.wait()
                self.active -= 1
            body = path.encode()
            writer.write(
                b"HTTP/1.1 200 OK\r\nContent-Length: "
                + str(len(body)).encode()
                + b"\r\nConnection: close\r\n\r\n"
                + body
            )
            await writer.drain()
        except (ConnectionError, asyncio.IncompleteReadError):
            # A gateway reset is measured at the client; this fixture still closes its socket.
            pass
        finally:
            writer.close()
            with contextlib.suppress(ConnectionError):
                await writer.wait_closed()
            self.backend_tasks.discard(task)

    async def request(self, name, retain=False):
        writer = None
        path = f"/-/file/icons/{name}.jpg"
        try:
            reader, writer = await asyncio.open_connection(
                "127.0.0.1", self.gateway_port
            )
            writer.write(
                (
                    f"GET {path} HTTP/1.1\r\nHost: cobalt-company.sakuin.org\r\nConnection: keep-alive\r\nAuthorization: Bearer fixture\r\nX-Wikijump-User-Id: 999\r\n\r\n"
                ).encode()
            )
            await writer.drain()
            status, body = await asyncio.wait_for(read_response(reader), 10)
            if retain:
                self.idle.append((reader, writer))
                writer = None
            return {"status": status, "body_ok": body == path.encode()}
        except (ConnectionError, asyncio.IncompleteReadError, TimeoutError) as error:
            return {"error": type(error).__name__}
        finally:
            if writer:
                writer.close()
                with contextlib.suppress(ConnectionError):
                    await writer.wait_closed()

    async def idle_closed(self, reader):
        try:
            return await asyncio.wait_for(reader.read(1), 0.2) == b""
        except TimeoutError:
            return False
        except ConnectionError:
            return True

    async def reuse_idle(self, index, reader, writer):
        path = f"/-/file/icons/reuse-{index}.jpg"
        try:
            writer.write(
                (
                    f"GET {path} HTTP/1.1\r\nHost: cobalt-company.sakuin.org\r\nConnection: keep-alive\r\n\r\n"
                ).encode()
            )
            await writer.drain()
            status, body = await asyncio.wait_for(read_response(reader), 5)
            return status == 200 and body == path.encode()
        except (ConnectionError, asyncio.IncompleteReadError, TimeoutError):
            return False

    async def run_scenario(self, image_requests, idle_connections):
        self.expected_images = image_requests
        for index in range(idle_connections):
            self.assertEqual(
                await self.request(f"idle-{index}", retain=True),
                {"status": 200, "body_ok": True},
            )
        requests = []
        for index in range(image_requests):
            requests.append(asyncio.create_task(self.request(f"image-{index}")))
            if index % 16 == 0:
                await asyncio.sleep(0)
        try:
            await asyncio.wait_for(self.all_arrived.wait(), 5)
        except TimeoutError:
            pass  # Release and collect rejected responses too; assertions below must fail.
        idle_closed = sum(
            await asyncio.gather(*(self.idle_closed(reader) for reader, _ in self.idle))
        )
        pressure_log = (self.root / "nginx.log").read_text()
        self.release.set()
        outcomes = await asyncio.gather(*requests)
        successful = sum(
            result == {"status": 200, "body_ok": True} for result in outcomes
        )
        reused = 0
        for index, (reader, writer) in enumerate(self.idle):
            reused += await self.reuse_idle(index, reader, writer)
        metrics = {
            "requests": image_requests,
            "successful": successful,
            "idle_connections": idle_connections,
            "idle_closed": idle_closed,
            "idle_reuse_success": reused,
            "upstream_peak": self.peak,
            "recycling_warnings": pressure_log.count("reusing connections"),
            "hard_limit_alerts": pressure_log.count("[alert]"),
            "statuses": {
                str(code): sum(r.get("status") == code for r in outcomes)
                for code in [200, 500, 502]
            },
            "transport_errors": sum("error" in r for r in outcomes),
        }
        print(json.dumps(metrics), flush=True)
        self.assertEqual(successful, image_requests, metrics)
        self.assertEqual(idle_closed, 0, metrics)
        self.assertEqual(reused, idle_connections, metrics)
        self.assertEqual(self.peak, image_requests, metrics)
        for headers in self.headers:
            self.assertEqual(headers["x-wikijump-site-id"].strip(), "6000000")
            self.assertNotIn("authorization", headers)
            self.assertNotIn("x-wikijump-user-id", headers)

    async def test_roster_burst_preserves_responses_and_idle_connections(self):
        await self.run_scenario(IMAGE_REQUESTS, IDLE_CONNECTIONS)

    async def test_active_proxy_requests_do_not_reclaim_idle_client_sockets(self):
        await self.run_scenario(80, 180)

    async def cleanup_fixture(self):
        self.release.set()
        for _, writer in self.idle:
            writer.close()
            with contextlib.suppress(ConnectionError):
                await writer.wait_closed()
        if self.upstream:
            self.upstream.close()
            await self.upstream.wait_closed()
        if self.backend_tasks:
            await asyncio.gather(*list(self.backend_tasks), return_exceptions=True)
        if self.nginx and self.nginx.poll() is None:
            self.nginx.terminate()
            try:
                await asyncio.to_thread(self.nginx.wait, 5)
            except subprocess.TimeoutExpired:
                os.killpg(self.nginx.pid, signal.SIGKILL)
                await asyncio.to_thread(self.nginx.wait)
        if self.log:
            self.log.close()
        self.temp.cleanup()
