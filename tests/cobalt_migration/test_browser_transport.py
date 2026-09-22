"""Synthetic CLI and JavaScript lifecycle tests; no browser or network."""

import importlib
import json
import subprocess
import tempfile
from pathlib import Path
import unittest


ORIGIN = "https://example.test"
HTML = "<div>private synthetic fixture</div>"

# Execute emitted JavaScript against a local fake browser, never real fetch.
NODE = r"""
const vm = require('node:vm');
const readline = require('node:readline');
const window = {};
let pending, request, aborted = false;
const timers = new Map();
let timerId = 0;
const context = vm.createContext({
  window, location: {origin: 'https://example.test'}, URL, AbortController,
  setTimeout: fn => {timers.set(++timerId, fn); return timerId;},
  clearTimeout: id => timers.delete(id),
  fetch: (path, options) => {
    request = {path, credentials: options.credentials, redirect: options.redirect};
    return new Promise((resolve, reject) => {
      pending = {resolve, reject};
      options.signal.addEventListener('abort', () => {
        aborted = true; reject(Object.assign(new Error('secret body'), {name:'AbortError'}));
      });
    });
  }
});
(async () => {
  for await (const line of readline.createInterface({input: process.stdin})) {
    const input = JSON.parse(line);
    if (input.origin) context.location.origin = input.origin;
    if (input.settle && pending) {
      if (input.settle === 'network') pending.reject(new Error('secret body'));
      else if (input.settle === 'timeout') for (const fn of timers.values()) fn();
      else if (input.settle === 'redirect') pending.resolve({status: 0, type: 'opaqueredirect',
        text: async () => {throw new Error('redirect body must not be read');}});
      else pending.resolve({status: 429, text: async () => '<div>private synthetic fixture</div>',
        headers: {get: name => name.toLowerCase() === 'retry-after' ? '7' : null}});
      pending = null;
      await new Promise(resolve => setImmediate(resolve));
    }
    let result;
    try { result = vm.runInContext(input.script, context); }
    catch (_) { result = {state: 'script_threw'}; }
    await new Promise(resolve => setImmediate(resolve));
    process.stdout.write(JSON.stringify({result, slots: Object.keys(window), request,
      aborted, timers: timers.size}) + '\n');
  }
})();
"""


class Clock:
    def __init__(self):
        self.value = 0.0

    def __call__(self):
        return self.value

    def sleep(self, seconds):
        self.value += seconds


class BrowserRunner:
    def __init__(self, settle="done", double_encoded=False, change_at=None):
        self.process = subprocess.Popen(
            ["node", "-e", NODE],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        self.settle = settle
        self.double_encoded = double_encoded
        self.change_at = change_at
        self.calls = 0
        self.observations = []

    def __call__(self, argv, *, capture_output, text, timeout):
        assert argv[:2] == ["browser-cli", "eval"]
        assert capture_output and text and timeout > 0
        self.calls += 1
        message = {"script": argv[2]}
        if self.calls == 3:
            message["settle"] = self.settle
        if self.calls == self.change_at:
            message["origin"] = "https://elsewhere.test"
        self.process.stdin.write(json.dumps(message) + "\n")
        self.process.stdin.flush()
        observed = json.loads(self.process.stdout.readline())
        self.observations.append(observed)
        output = json.dumps(observed["result"])
        if self.double_encoded:
            output = json.dumps(output)
        return subprocess.CompletedProcess(argv, 0, output, "")

    def close(self):
        self.process.stdin.close()
        self.process.wait(timeout=3)
        self.process.stdout.close()
        self.process.stderr.close()


class BrowserTransportTests(unittest.TestCase):
    def factory(self, **options):
        try:
            module = importlib.import_module("tools.cobalt_migration.browser_transport")
        except ModuleNotFoundError:
            self.fail("browser transport module is not implemented")
        return module.make_browser_fetch(ORIGIN, **options)

    def runner(self, **options):
        runner = BrowserRunner(**options)
        self.addCleanup(runner.close)
        return runner

    def fetch(self, runner):
        clock = Clock()
        return self.factory(
            runner=runner,
            clock=clock,
            sleep=clock.sleep,
            timeout_seconds=1,
            poll_interval=0.25,
        )

    def test_start_pending_done_and_cleanup_for_both_cli_json_encodings(self):
        for double_encoded in (False, True):
            with self.subTest(double_encoded=double_encoded):
                runner = self.runner(double_encoded=double_encoded)
                response = self.fetch(runner)("/pagelist/p/2")
                self.assertEqual(
                    (response.status, response.html, response.retry_after),
                    (429, HTML, "7"),
                )
                self.assertEqual(
                    runner.observations[0]["request"],
                    {
                        "path": "/pagelist/p/2",
                        "credentials": "same-origin",
                        "redirect": "manual",
                    },
                )
                self.assertEqual(len(runner.observations[0]["slots"]), 1)
                self.assertEqual(runner.observations[1]["result"]["state"], "pending")
                self.assertEqual(runner.observations[-1]["slots"], [])
                self.assertEqual(runner.observations[-1]["timers"], 0)

    def test_opaque_redirect_permanently_stops_listing_without_checkpoint(self):
        from tools.cobalt_migration.listing_export import export_listing

        runner = self.runner(settle="redirect")
        with tempfile.TemporaryDirectory() as directory:
            checkpoint = Path(directory) / "listing.json"
            delays = []
            with self.assertRaises(RuntimeError) as raised:
                export_listing(
                    ORIGIN, checkpoint, self.fetch(runner), sleep=delays.append
                )
            self.assertEqual(type(raised.exception).__name__, "SourcePageRedirect")
            self.assertFalse(checkpoint.exists())
        self.assertEqual(delays, [])
        self.assertEqual(runner.calls, 4)
        self.assertEqual(runner.observations[-1]["slots"], [])
        self.assertEqual(runner.observations[-1]["timers"], 0)

    def test_unique_slots_between_fetches(self):
        first, second = self.runner(), self.runner()
        self.fetch(first)("/one")
        self.fetch(second)("/two")
        self.assertNotEqual(
            first.observations[0]["slots"], second.observations[0]["slots"]
        )

    def test_network_failure_is_retryable_and_sanitized(self):
        runner = self.runner(settle="network")
        with self.assertRaises(ConnectionError) as raised:
            self.fetch(runner)("/one")
        self.assertNotIn("secret", str(raised.exception))
        self.assertEqual(runner.observations[-1]["slots"], [])

    def test_browser_timeout_is_retryable_and_cleans_slot(self):
        runner = self.runner(settle="timeout")
        with self.assertRaises(TimeoutError):
            self.fetch(runner)("/one")
        self.assertTrue(runner.observations[-1]["aborted"])
        self.assertEqual(runner.observations[-1]["slots"], [])

    def test_poll_deadline_aborts_pending_fetch(self):
        runner = self.runner(settle=None)
        with self.assertRaises(TimeoutError):
            self.fetch(runner)("/one")
        self.assertTrue(runner.observations[-1]["aborted"])
        self.assertEqual(runner.observations[-1]["slots"], [])
        self.assertLess(runner.calls, 10)

    def test_wrong_origin_before_start_does_not_fetch(self):
        runner = self.runner(change_at=1)
        with self.assertRaisesRegex(RuntimeError, "origin"):
            self.fetch(runner)("/one")
        self.assertNotIn("request", runner.observations[0])
        self.assertEqual(runner.observations[0]["slots"], [])

    def test_origin_change_during_poll_and_cleanup_fails_closed(self):
        runner = self.runner(change_at=2)
        with self.assertRaisesRegex(RuntimeError, "origin") as raised:
            self.fetch(runner)("/one")
        self.assertTrue(any("cleanup" in note for note in raised.exception.__notes__))
        self.assertEqual(len(runner.observations[-1]["slots"]), 1)

    def test_cleanup_failure_after_success_is_explicit(self):
        runner = self.runner(change_at=4)
        with self.assertRaisesRegex(RuntimeError, "cleanup"):
            self.fetch(runner)("/one")

    def test_cleanup_failure_preserves_network_error(self):
        runner = self.runner(settle="network", change_at=4)
        with self.assertRaises(ConnectionError) as raised:
            self.fetch(runner)("/one")
        self.assertTrue(any("cleanup" in note for note in raised.exception.__notes__))

    def test_invalid_paths_never_invoke_runner(self):
        def forbidden(*args, **kwargs):
            self.fail("runner called for invalid input")

        fetch = self.fetch(forbidden)
        for path in (
            "https://elsewhere.test/x",
            "//elsewhere.test/x",
            "relative",
            "/\\evil",
            "/x\n",
            "/x#y",
            "",
        ):
            with self.subTest(path=path), self.assertRaises(ValueError):
                fetch(path)

    def test_tool_and_protocol_errors_never_expose_captured_bodies(self):
        failures = [
            subprocess.CompletedProcess([], 1, HTML, HTML),
            subprocess.CompletedProcess([], 0, HTML, ""),
            subprocess.CompletedProcess(
                [], 0, json.dumps({"state": "done", "status": "200", "html": HTML}), ""
            ),
            subprocess.TimeoutExpired(["browser-cli"], 1, output=HTML, stderr=HTML),
            OSError(HTML),
        ]
        for failure in failures:
            with self.subTest(failure=type(failure).__name__):
                calls = []

                def runner(argv, **kwargs):
                    calls.append(argv)
                    if len(calls) > 1:
                        return subprocess.CompletedProcess(
                            argv, 0, '{"state":"cleaned"}', ""
                        )
                    if isinstance(failure, Exception):
                        raise failure
                    return failure

                with self.assertRaises((RuntimeError, TimeoutError)) as raised:
                    self.fetch(runner)("/one")
                self.assertNotIn(HTML, str(raised.exception))
                self.assertTrue(raised.exception.__suppress_context__)
                self.assertEqual(len(calls), 2)


if __name__ == "__main__":
    unittest.main()
