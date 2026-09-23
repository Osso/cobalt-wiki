"""Read-only Wikidot history modules through one explicitly pinned CDP page target."""

import json
import math
import re
import subprocess
from urllib.parse import urlsplit

from .history_export import HistoryResponse


class HistoryTransportError(RuntimeError):
    """The pinned page, source module, or CDP protocol is unsafe to continue."""


NODE_EVAL = r"""
const {source_origin, cdp_origin, target_id, request, timeout_ms} = JSON.parse(
  await new Promise((resolve, reject) => {
    let input = '';
    process.stdin.setEncoding('utf8');
    process.stdin.on('data', chunk => { input += chunk; });
    process.stdin.on('end', () => resolve(input));
    process.stdin.on('error', reject);
  })
);
const output = result => process.stdout.write(JSON.stringify(result));
let socket, timer;
try {
  const listing = await fetch(cdp_origin + '/json', {
    signal: AbortSignal.timeout(timeout_ms)
  }).then(response => response.json());
  if (!Array.isArray(listing)) throw new Error('invalid targets');
  const target = listing.find(page => page.id === target_id && page.type === 'page');
  if (!target) { output({state:'target_missing'}); process.exit(0); }
  if (new URL(target.url).origin !== source_origin) {
    output({state:'origin_changed'}); process.exit(0);
  }
  const websocket = new URL(target.webSocketDebuggerUrl);
  const endpoint = new URL(cdp_origin);
  if (websocket.protocol !== 'ws:' || websocket.host !== endpoint.host ||
      websocket.username || websocket.password) {
    output({state:'protocol_error'}); process.exit(0);
  }
  socket = new WebSocket(websocket);
  const expression = `(async () => {
    if (location.origin !== ${JSON.stringify(source_origin)}) return {state:'origin_changed'};
    if (typeof jQuery?.param !== 'function' ||
        typeof OZONE?.ajax?.parseResponse !== 'function') return {state:'protocol_error'};
    const cookie = document.cookie.split(';').map(value => value.trim())
      .find(value => value.startsWith('wikidot_token7='));
    if (!cookie) return {state:'module_error'};
    const request = ${JSON.stringify(request)};
    if (request.options) request.options = JSON.stringify(request.options);
    const token = cookie.slice('wikidot_token7='.length);
    const body = jQuery.param({...request, wikidot_token7: token, callbackIndex: 0});
    let response;
    try {
      response = await fetch('/ajax-module-connector.php', {
        method:'POST', credentials:'same-origin', redirect:'manual',
        headers:{'Content-Type':'application/x-www-form-urlencoded; charset=UTF-8'},
        body, signal:AbortSignal.timeout(${timeout_ms})
      });
    } catch (error) {
      if (location.origin !== ${JSON.stringify(source_origin)}) return {state:'origin_changed'};
      return {state:error.name === 'TimeoutError' || error.name === 'AbortError'
        ? 'timeout' : 'network_error'};
    }
    if (location.origin !== ${JSON.stringify(source_origin)}) return {state:'origin_changed'};
    if (response.type === 'opaqueredirect') return {state:'redirect'};
    let raw;
    try { raw = await response.text(); }
    catch (error) {
      if (location.origin !== ${JSON.stringify(source_origin)}) return {state:'origin_changed'};
      return {state:error.name === 'TimeoutError' || error.name === 'AbortError'
        ? 'timeout' : 'network_error'};
    }
    const retry_after = response.headers.get('Retry-After');
    if (response.status !== 200) return {
      state:'done', status:response.status, raw, html:'', retry_after
    };
    try {
      const parsed = OZONE.ajax.parseResponse(raw);
      return {state:'done', status:response.status, raw,
        html:parsed?.status === 'ok' && typeof parsed.body === 'string'
          ? parsed.body : '', retry_after};
    } catch (_) {
      return {state:'done', status:response.status, raw, html:'', retry_after};
    }
  })()`;
  const result = await Promise.race([
    new Promise((resolve, reject) => {
      socket.addEventListener('open', () => socket.send(JSON.stringify({
        id:1, method:'Runtime.evaluate', params:{expression, awaitPromise:true,
          returnByValue:true}
      })), {once:true});
      socket.addEventListener('message', event => {
        try {
          const message = JSON.parse(event.data);
          if (message.id !== 1) return;
          if (message.error || message.result?.exceptionDetails) {
            resolve({state:'protocol_error'});
          } else {
            resolve(message.result?.result?.value ?? {state:'protocol_error'});
          }
        } catch (_) { resolve({state:'protocol_error'}); }
      });
      socket.addEventListener('error', reject, {once:true});
      socket.addEventListener('close', () => reject(new Error('target closed')), {once:true});
    }),
    new Promise(resolve => { timer = setTimeout(() => resolve({state:'timeout'}), timeout_ms); })
  ]);
  output(result);
} catch (error) {
  output({state:error.name === 'TimeoutError' ? 'timeout' : 'network_error'});
} finally {
  clearTimeout(timer);
  if (socket) socket.close();
}
"""


def _origin(value, *, cdp=False):
    try:
        parts = urlsplit(value)
        valid = (
            isinstance(value, str)
            and parts.scheme == ("http" if cdp else "https")
            and parts.hostname
            and parts.username is None
            and parts.password is None
            and not parts.path
            and not parts.query
            and not parts.fragment
            and parts.port != 0
            and "\\" not in value
            and not any(
                character.isspace() or ord(character) < 32 for character in value
            )
        )
        if cdp:
            valid = valid and parts.hostname in {"127.0.0.1", "localhost"}
    except (TypeError, ValueError):
        valid = False
    if not valid:
        raise ValueError("invalid CDP origin" if cdp else "invalid source origin")


def _request(request):
    if not isinstance(request, dict):
        raise ValueError("invalid history module request")
    module = request.get("moduleName")
    if module == "history/PageRevisionListModule":
        valid = (
            set(request) == {"moduleName", "page", "perpage", "page_id", "options"}
            and type(request["page"]) is int
            and request["page"] > 0
            and type(request["perpage"]) is int
            and request["perpage"] == 20
            and type(request["page_id"]) is int
            and request["page_id"] > 0
            and request["options"] == {"all": True}
        )
    elif module == "history/PageSourceModule":
        valid = (
            set(request) == {"moduleName", "revision_id"}
            and type(request["revision_id"]) is int
            and request["revision_id"] > 0
        )
    else:
        valid = False
    if not valid:
        raise ValueError("invalid history module request")


class HistoryFetch:
    def __init__(
        self,
        source_origin,
        target_id,
        *,
        node_binary="node",
        cdp_origin="http://127.0.0.1:9222",
        runner=subprocess.run,
        timeout_seconds=30,
    ):
        _origin(source_origin)
        _origin(cdp_origin, cdp=True)
        if not isinstance(target_id, str) or not re.fullmatch(
            r"[a-fA-F0-9]{32}", target_id
        ):
            raise ValueError("invalid CDP target ID")
        if not isinstance(node_binary, str) or not node_binary or "\0" in node_binary:
            raise ValueError("invalid Node executable")
        if (
            not isinstance(timeout_seconds, (int, float))
            or not math.isfinite(timeout_seconds)
            or not 0 < timeout_seconds <= 2147483
        ):
            raise ValueError("invalid history fetch timeout")
        self.source_origin = source_origin
        self.target_id = target_id
        self.node_binary = node_binary
        self.cdp_origin = cdp_origin
        self.runner = runner
        self.timeout_seconds = timeout_seconds

    def __call__(self, request):
        _request(request)
        payload = json.dumps(
            {
                "source_origin": self.source_origin,
                "cdp_origin": self.cdp_origin,
                "target_id": self.target_id,
                "request": request,
                "timeout_ms": math.ceil(self.timeout_seconds * 1000),
            }
        )
        try:
            result = self.runner(
                [self.node_binary, "-e", NODE_EVAL],
                input=payload,
                capture_output=True,
                text=True,
                timeout=self.timeout_seconds + 2,
            )
        except subprocess.TimeoutExpired:
            raise TimeoutError("history CDP deadline exceeded") from None
        except OSError:
            raise ConnectionError("history CDP process unavailable") from None
        if result.returncode != 0:
            raise HistoryTransportError("history CDP process failed") from None
        try:
            response = json.loads(result.stdout)
        except (ValueError, TypeError):
            raise HistoryTransportError("invalid history CDP response") from None
        if not isinstance(response, dict):
            raise HistoryTransportError("invalid history CDP response") from None
        state = response.get("state")
        if state == "timeout":
            raise TimeoutError("history CDP deadline exceeded") from None
        if state == "network_error":
            raise ConnectionError("history CDP connection failed") from None
        if state != "done":
            raise HistoryTransportError(
                f"history CDP {state if state in {'origin_changed', 'target_missing', 'redirect', 'module_error', 'protocol_error'} else 'invalid response'}"
            ) from None
        status, raw, html, retry_after = (
            response.get("status"),
            response.get("raw"),
            response.get("html"),
            response.get("retry_after"),
        )
        if (
            type(status) is not int
            or not 100 <= status <= 599
            or not isinstance(raw, str)
            or not isinstance(html, str)
            or not (retry_after is None or isinstance(retry_after, str))
        ):
            raise HistoryTransportError("invalid history CDP response") from None
        return HistoryResponse(status, raw, html, retry_after)


def make_history_fetch(source_origin, target_id, **options):
    """Return a read-only module fetcher pinned to one CDP target ID."""
    return HistoryFetch(source_origin, target_id, **options)
