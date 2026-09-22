"""Keep dependent services waiting until Deepwell serves its JSON-RPC API."""

from email.utils import parsedate_to_datetime
import json
import random
import sys
import time
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


def retry_after(error):
    if not isinstance(error, HTTPError):
        return 0
    try:
        if error.code != 429 and not 500 <= error.code < 600:
            raise error
        value = error.headers.get("Retry-After")
        if not value:
            return 0
        if value.isdigit():
            return float(value)
        return max(0, parsedate_to_datetime(value).timestamp() - time.time())
    finally:
        error.close()


def wait_for_deepwell(endpoint="http://127.0.0.1:2747"):
    request = Request(
        endpoint,
        data=json.dumps(
            {"jsonrpc": "2.0", "id": 1, "method": "ping", "params": []}
        ).encode(),
        headers={"Content-Type": "application/json"},
    )
    for attempt in range(6):
        try:
            with urlopen(request, timeout=3) as response:
                result = json.load(response)
            if (
                "error" in result
                or not isinstance(result.get("result"), str)
                or not result["result"]
            ):
                raise RuntimeError(
                    "Deepwell readiness ping returned an invalid response"
                )
            return
        except (URLError, TimeoutError, ConnectionError) as error:
            delay = retry_after(error)
            if attempt == 5:
                raise RuntimeError(
                    "Deepwell API did not become ready after six attempts"
                ) from error
            print(
                f"Waiting for Deepwell API (attempt {attempt + 1}/6)", file=sys.stderr
            )
            time.sleep(max(delay, 2**attempt + random.random()))


if __name__ == "__main__":
    wait_for_deepwell()
