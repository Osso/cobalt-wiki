"""Plan and reconcile latest-source POC imports through trusted Deepwell RPCs.

No SQL, compression, S3 addressing, or revision internals are reimplemented.
Source metadata remains supplemental evidence, not target creation history.
"""

import argparse
import hashlib
import ipaddress
import json
from pathlib import Path
import random
import tarfile
import time
from urllib.error import HTTPError, URLError
from urllib.parse import urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener

from .archive import inventory_archive, write_manifest
from .listing_export import _prepare_path, _retry_delay


class PocImportError(ValueError):
    """An import cannot proceed without changing or inventing source data."""


def _digest(value):
    return hashlib.sha256(
        json.dumps(
            value, sort_keys=True, ensure_ascii=True, separators=(",", ":")
        ).encode()
    ).hexdigest()


def _canonical_keys(listing):
    if listing.get("completed_page") != listing.get("highest_page"):
        raise PocImportError("canonical listing is incomplete")
    names = listing.get("fullnames")
    if not isinstance(names, list) or not names:
        raise PocImportError("canonical listing has no names")
    keys = {}
    for name in names:
        if (
            not isinstance(name, str)
            or not name
            or name in {".", ".."}
            or any(c.isspace() or ord(c) < 32 or c in "/\\" for c in name)
        ):
            raise PocImportError("invalid canonical identity")
        key = name.replace(":", "_")
        if key in keys:
            raise PocImportError("canonical identities collide in archive encoding")
        keys[key] = name
    return keys


def prepare_plan(archive_path, listing, metadata, output_path, *, site_id, user_id):
    """Validate the entire archive and write an immutable, protected import plan."""
    if type(site_id) is not int or site_id <= 0:
        raise PocImportError("explicit positive target site ID required")
    if type(user_id) is not int or (user_id != -1 and user_id <= 0):
        raise PocImportError("technical user ID must be -1 or a positive integer")
    keys = _canonical_keys(listing)
    evidence = {}
    names = set(keys.values())
    for record in metadata.get("records", []):
        name = record.get("fullname")
        if name not in names or name in evidence:
            raise PocImportError(
                "metadata identities are duplicate or outside the listing"
            )
        evidence[name] = record
    manifest = inventory_archive(archive_path)
    pages, attachments = [], []
    source_keys = set()
    for entry in manifest["files"]:
        path = entry["path"]
        if entry["role"] == "page_source":
            key = path.removeprefix("source/").removesuffix(".txt")
            if not path.endswith(".txt") or key not in keys or key in source_keys:
                raise PocImportError(
                    "archive source does not bijectively match listing"
                )
            source_keys.add(key)
            name = keys[key]
            record = evidence.get(name, {"status": "not_acquired"})
            accepted = record["status"] == "accepted"
            title = record.get("title") if accepted else name
            tags = record.get("tags") if accepted else []
            if (
                not isinstance(title, str)
                or not isinstance(tags, list)
                or any(not isinstance(tag, str) for tag in tags)
            ):
                raise PocImportError("invalid accepted title or tags")
            pages.append(
                {
                    **entry,
                    "fullname": name,
                    "title": title,
                    "tags": tags,
                    "metadata_status": record["status"],
                    "metadata": record,
                }
            )
        else:
            key, separator, filename = path.removeprefix("files/").partition("/")
            if not separator or key not in keys or not filename or "/" in filename:
                raise PocImportError(
                    "attachment owner/name cannot be mapped losslessly"
                )
            attachments.append({**entry, "fullname": keys[key], "name": filename})
    if source_keys != set(keys):
        raise PocImportError("listing contains pages absent from archive")
    plan = {
        "schema": 1,
        "archive_sha256": manifest["archive_sha256"],
        "site_id": site_id,
        "user_id": user_id,
        "pages": pages,
        "attachments": attachments,
        "attribution": "Technical import principal; source creators/history unknown.",
        "timestamps": "Target creation times are import times, not source creation times.",
        "missing_metadata": "Canonical fullname is a POC label; tags remain unacquired.",
    }
    plan["plan_sha256"] = _digest(plan)
    output = Path(output_path)
    _prepare_path(output)
    if output.exists() and json.loads(output.read_text()) != plan:
        raise PocImportError("refusing to replace a different import plan")
    write_manifest(plan, output)
    return plan


def _load_plan(archive_path, plan_path):
    path = Path(plan_path)
    _prepare_path(path)
    plan = json.loads(path.read_text())
    unsigned = {key: value for key, value in plan.items() if key != "plan_sha256"}
    if plan.get("schema") != 1 or plan.get("plan_sha256") != _digest(unsigned):
        raise PocImportError("import plan digest/schema mismatch")
    with Path(archive_path).open("rb") as source:
        actual = hashlib.file_digest(source, "sha256").hexdigest()
    if actual != plan["archive_sha256"]:
        raise PocImportError("archive changed since planning")
    return plan


def _marker(plan):
    return (
        "Cobalt POC import "
        + plan["plan_sha256"]
        + "; source authorship/history unacquired"
    )


def _owns(record, plan):
    if (
        record.get("revision_comments") != _marker(plan)
        or record.get("revision_user_id") != plan["user_id"]
    ):
        raise PocImportError("target record is not owned by this import plan")


def _matches_bytes(data, entry):
    if (
        len(data) != entry["size"]
        or hashlib.sha256(data).hexdigest() != entry["sha256"]
    ):
        raise PocImportError("target bytes differ from planned archive member")


def _get_page(rpc, plan, entry):
    return rpc(
        "page_get",
        {
            "site_id": plan["site_id"],
            "page": entry["fullname"],
            "details": {"wikitext": True},
        },
    )


def _check_page(page, plan, entry):
    _owns(page, plan)
    if page.get("slug") != entry["fullname"] or page.get("title") != entry["title"]:
        raise PocImportError("target page identity/title differs from plan")
    source = page.get("wikitext")
    if not isinstance(source, str):
        raise PocImportError("target did not return raw page source")
    _matches_bytes(source.encode("utf-8"), entry)
    if page.get("tags") not in ([], entry["tags"]):
        raise PocImportError("target tags differ from plan")


def _get_file(rpc, plan, entry, page_id):
    return rpc(
        "file_get",
        {
            "site_id": plan["site_id"],
            "page_id": page_id,
            "file": entry["name"],
            "details": {"data": True},
        },
    )


def _check_file(file, plan, entry):
    _owns(file, plan)
    if file.get("name") != entry["name"]:
        raise PocImportError("target attachment name differs from plan")
    try:
        data = bytes.fromhex(file["data"])
    except (KeyError, ValueError, TypeError):
        raise PocImportError("target did not return attachment bytes") from None
    _matches_bytes(data, entry)


def _preflight(rpc, plan):
    """Refuse existing conflicting targets before any mutation in this invocation."""
    pages = {}
    for entry in plan["pages"]:
        page = _get_page(rpc, plan, entry)
        if page is not None:
            _check_page(page, plan, entry)
            pages[entry["fullname"]] = page["page_id"]
    for entry in plan["attachments"]:
        if entry["fullname"] in pages:
            file = _get_file(rpc, plan, entry, pages[entry["fullname"]])
            if file is not None:
                _check_file(file, plan, entry)
    return pages


def _members(archive_path, entries):
    remaining = {entry["path"]: entry for entry in entries}
    with tarfile.open(archive_path, "r|gz") as archive:
        for member in archive:
            entry = remaining.pop(member.name, None)
            if entry is not None:
                content = archive.extractfile(member)
                if content is None:
                    raise PocImportError("planned archive member is not a file")
                data = content.read()
                _matches_bytes(data, entry)
                yield entry, data
    if remaining:
        raise PocImportError("planned archive members missing")


def _import_page(rpc, plan, entry, data):
    page = _get_page(rpc, plan, entry)
    if page is None:
        rpc(
            "page_import",
            {
                "site_id": plan["site_id"],
                "user_id": plan["user_id"],
                "slug": entry["fullname"],
                "title": entry["title"],
                "wikitext": data.decode("utf-8"),
                "alt_title": None,
                "layout": None,
                "revision_comments": _marker(plan),
                "bypass_filter": True,
                "ip_address": "127.0.0.1",
            },
        )
        page = _get_page(rpc, plan, entry)
    if page is None:
        raise PocImportError("created page not found under exact canonical identity")
    _check_page(page, plan, entry)
    if page["tags"] != entry["tags"]:
        rpc(
            "page_edit",
            {
                "site_id": plan["site_id"],
                "user_id": plan["user_id"],
                "page": page["page_id"],
                "last_revision_id": page["revision_id"],
                "tags": entry["tags"],
                "revision_comments": _marker(plan),
                "ip_address": "127.0.0.1",
            },
        )
        page = _get_page(rpc, plan, entry)
        _check_page(page, plan, entry)
        if page["tags"] != entry["tags"]:
            raise PocImportError("target failed to retain exact tags")
    return page["page_id"]


def _import_file(rpc, put, plan, entry, data, page_id):
    file = _get_file(rpc, plan, entry, page_id)
    if file is None:
        upload = rpc(
            "blob_upload", {"user_id": plan["user_id"], "blob_size": len(data)}
        )
        put(upload["presign_url"], data)
        rpc(
            "file_create",
            {
                "site_id": plan["site_id"],
                "page_id": page_id,
                "user_id": plan["user_id"],
                "name": entry["name"],
                "uploaded_blob_id": upload["pending_blob_id"],
                "revision_comments": _marker(plan),
                "bypass_filter": True,
                "ip_address": "127.0.0.1",
            },
        )
        file = _get_file(rpc, plan, entry, page_id)
    if file is None:
        raise PocImportError("created attachment not found")
    _check_file(file, plan, entry)


def apply_plan(archive_path, plan_path, rpc, put):
    """Apply/reconcile an immutable plan. RPC mutations are never blindly retried.

    Restart reads committed records, verifies ownership and bytes, and only
    creates absent records. Run exclusively against a provisioned, gated site.
    """
    plan = _load_plan(archive_path, plan_path)
    pages = _preflight(rpc, plan)
    for entry, data in _members(archive_path, plan["pages"]):
        pages[entry["fullname"]] = _import_page(rpc, plan, entry, data)
    for entry, data in _members(archive_path, plan["attachments"]):
        _import_file(rpc, put, plan, entry, data, pages[entry["fullname"]])
    return {"pages": len(pages), "attachments": len(plan["attachments"])}


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, msg, headers, newurl):
        return None


def _loopback_url(url):
    parts = urlsplit(url)
    try:
        valid = ipaddress.ip_address(parts.hostname).is_loopback
    except ValueError:
        valid = False
    if (
        parts.scheme != "http"
        or not valid
        or parts.username
        or parts.password
        or parts.fragment
    ):
        raise PocImportError("transport requires an explicit HTTP loopback IP endpoint")


class LoopbackRpc:
    """Use through an SSH forward or on the target; tokens never enter CLI argv."""

    def __init__(self, endpoint, session_token, site_id):
        _loopback_url(endpoint)
        self.endpoint, self.token, self.site_id = endpoint, session_token, site_id
        self.opener = build_opener(_NoRedirect())

    def _request(self, request, safe):
        for attempt in range(4 if safe else 1):
            try:
                with self.opener.open(request, timeout=60) as response:
                    return response.read()
            except HTTPError as error:
                retry = error.code == 429 or 500 <= error.code < 600
                try:
                    delay = _retry_delay(error.headers.get("Retry-After"), time.time)
                finally:
                    error.close()
            except (URLError, TimeoutError, ConnectionError):
                retry, delay = True, 0
            if not safe or not retry or attempt == 3:
                raise PocImportError(
                    "transport failed; reconcile by restarting the same plan"
                ) from None
            time.sleep(max(delay, 2**attempt + random.random()))
        raise AssertionError("unreachable")

    def rpc(self, method, params):
        headers = {
            "Content-Type": "application/json",
            "X-Deepwell-Session-Token": self.token,
            "X-Deepwell-Site-Id": str(self.site_id),
        }
        if method == "page_edit" and isinstance(params, dict) and "page" in params:
            headers["X-Deepwell-Page"] = str(params["page"])
        request = Request(
            self.endpoint,
            data=json.dumps(
                {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
            ).encode(),
            headers=headers,
        )
        response = json.loads(
            self._request(
                request,
                method
                in {
                    "page_get",
                    "file_get",
                    "session_get",
                    "page_import",
                    "file_create",
                },
            )
        )
        if "error" in response or "result" not in response:
            raise PocImportError(
                f"Deepwell rejected {method}; inspect protected server logs"
            )
        return response["result"]

    def put(self, url, data):
        _loopback_url(url)
        self._request(Request(url, data=data, method="PUT"), True)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prepare = commands.add_parser("plan")
    prepare.add_argument("--archive", required=True)
    prepare.add_argument("--listing", required=True)
    prepare.add_argument("--metadata", required=True)
    prepare.add_argument("--output", required=True)
    prepare.add_argument("--site-id", type=int, required=True)
    prepare.add_argument("--user-id", type=int, required=True)
    apply = commands.add_parser("apply")
    apply.add_argument("--archive", required=True)
    apply.add_argument("--plan", required=True)
    apply.add_argument("--endpoint", required=True)
    apply.add_argument("--session-file", required=True)
    args = parser.parse_args(argv)
    if args.command == "plan":
        plan = prepare_plan(
            args.archive,
            json.loads(Path(args.listing).read_text()),
            json.loads(Path(args.metadata).read_text()),
            args.output,
            site_id=args.site_id,
            user_id=args.user_id,
        )
        result = {"pages": len(plan["pages"]), "attachments": len(plan["attachments"])}
    else:
        plan = _load_plan(args.archive, args.plan)
        session_path = Path(args.session_file)
        _prepare_path(session_path)
        token = session_path.read_text().strip()
        client = LoopbackRpc(args.endpoint, token, plan["site_id"])
        session = client.rpc("session_get", [token])
        if not session or session.get("user_id") != plan["user_id"]:
            raise PocImportError(
                "session does not belong to technical import principal"
            )
        result = apply_plan(args.archive, args.plan, client.rpc, client.put)
    print(json.dumps(result))


if __name__ == "__main__":
    main()
