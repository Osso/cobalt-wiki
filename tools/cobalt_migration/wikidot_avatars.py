"""Give imported members the avatar they had on Wikidot.

Imported members keep their Wikidot user ID as their Cobalt user ID. Locally
created accounts also have positive IDs, which can belong to an unrelated
Wikidot user, so a member is imported only when their Wikidot profile (by
slug) reports that same ID. Members who already have an avatar, Wikidot's
shared default image, non-images and files over Deepwell's avatar limit are
skipped. Without --apply nothing is written: each member's plan is printed.

    python -m tools.cobalt_migration.wikidot_avatars ENDPOINT SITE_ID PASSWORD_FILE
    ... --apply

Reruns are safe: a member with an avatar is never overwritten. Uploads run
at most one per --interval seconds.
"""

import argparse
import random
import re
import sys
import time
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlsplit
from urllib.request import Request, urlopen

from .poc_import import LoopbackRpc

IP_ADDRESS = "127.0.0.1"
# Deepwell's `maximum-avatar-size-kb` (250 KiB).
MAX_AVATAR_BYTES = 250 * 1024
PROFILE_URL = "https://www.wikidot.com/user:info/{slug}"
AVATAR_URL = "https://www.wikidot.com/avatar.php?userid={user_id}&size=normal"
PROFILE_USER_ID = re.compile(rb"userId = (\d+)")


class Wikidot:
    """Public Wikidot reads, retrying 429/5xx and network errors with backoff."""

    def __init__(self, sleep=time.sleep):
        self.sleep = sleep

    def _get(self, url, limit):
        """Returns (final URL, content type, bytes), or None for 404."""
        request = Request(url, headers={"User-Agent": "cobalt-wikidot-avatars"})
        for attempt in range(4):
            try:
                with urlopen(request, timeout=30) as response:
                    return response.url, response.headers.get_content_type(), response.read(limit)
            except HTTPError as error:
                error.close()
                if error.code == 404:
                    return None
                if error.code != 429 and error.code < 500 or attempt == 3:
                    raise
                retry_after = error.headers.get("Retry-After", "")
                delay = int(retry_after) if retry_after.isdigit() else 0
            except (URLError, TimeoutError):
                if attempt == 3:
                    raise
                delay = 0
            self.sleep(max(delay, 2**attempt + random.random()))
        raise AssertionError("unreachable")

    def user_id(self, slug):
        """The Wikidot user ID on the profile page for `slug`, if any."""
        found = self._get(PROFILE_URL.format(slug=quote(slug)), 1024 * 1024)
        match = found and PROFILE_USER_ID.search(found[2])
        return int(match.group(1)) if match else None

    def avatar(self, user_id):
        return self._get(AVATAR_URL.format(user_id=user_id), MAX_AVATAR_BYTES + 1)


def plan_member(member, current, wikidot):
    """Returns (description, avatar bytes to import or None)."""
    user_id = member["user_id"]
    if current["avatar_s3_hash"]:
        return "already has an avatar, skipped", None
    wikidot_id = wikidot.user_id(member["slug"])
    if wikidot_id is None:
        return "no Wikidot account with this slug, skipped", None
    if wikidot_id != user_id:
        return f"Wikidot account {wikidot_id} does not match user {user_id}, skipped", None
    found = wikidot.avatar(user_id)
    if found is None:
        return "no Wikidot avatar, skipped", None
    url, content_type, data = found
    if urlsplit(url).path.rstrip("/").endswith("/default_avatar"):
        return "Wikidot default avatar, skipped", None
    if not content_type.startswith("image/"):
        return f"Wikidot returned {content_type}, skipped", None
    if len(data) > MAX_AVATAR_BYTES:
        return f"Wikidot avatar is {len(data)} bytes, over the limit, skipped", None
    return f"import Wikidot avatar ({len(data)} bytes, {content_type})", data


def store_avatar(rpc, user_id, data):
    """Uploads as the member: Deepwell requires the avatar's own uploader."""
    upload = rpc.rpc("blob_upload", {"user_id": user_id, "blob_size": len(data)})
    rpc.put(upload["presign_url"], data)
    rpc.rpc("user_edit", {
        "user": user_id,
        "avatar_uploaded_blob_id": upload["pending_blob_id"],
        "ip_address": IP_ADDRESS,
    })


def run(rpc, wikidot, write, interval, sleep=time.sleep):
    """Processes site members in slug order, yielding a line for each."""
    members = sorted(rpc.rpc("member_admin_list", {}), key=lambda member: member["slug"])
    for member in members:
        if member["user_id"] <= 0:
            description, data = "no Wikidot account, skipped", None
        else:
            current = rpc.rpc("user_get", {"user": member["user_id"]})
            description, data = plan_member(member, current, wikidot)
        line = f"{member['slug']}: {description}"
        stored = write and data is not None
        if stored:
            store_avatar(rpc, member["user_id"], data)
        yield line if write else f"would {line}"
        if stored:
            sleep(interval)


def parse_args(argv):
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("endpoint")
    parser.add_argument("site_id", type=int)
    parser.add_argument("password_file")
    parser.add_argument("--apply", action="store_true", help="store the avatars")
    parser.add_argument("--interval", type=float, default=1.0)
    return parser.parse_args(argv)


def main(argv):
    args = parse_args(argv)
    with open(args.password_file) as file:
        password = file.read().strip()
    login = LoopbackRpc(args.endpoint, "", args.site_id).rpc("login", {
        "name_or_email": "cobalt-import", "password": password,
        "ip_address": IP_ADDRESS, "user_agent": "cobalt-wikidot-avatars",
    })
    rpc = LoopbackRpc(args.endpoint, login["session_token"], args.site_id)
    for line in run(rpc, Wikidot(), args.apply, args.interval):
        print(line, flush=True)


if __name__ == "__main__":
    main(sys.argv[1:])
