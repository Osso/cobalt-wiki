"""Import the Wikidot accounts that authored the site's revisions.

Authors (ID, slug, display name) come from the revision list written by
wikidot_changes.py; each public profile (www.wikidot.com/user:info/<slug>)
gives the join time, karma level and account type. Records are created with
Deepwell's import_wikidot_user as the import principal, so page attribution
and votes can refer to these accounts and people can later claim them.

    python -m tools.cobalt_migration.wikidot_users CHANGES_JSON \\
        http://127.0.0.1:27471/jsonrpc 6000000 PASSWORD_FILE
"""

import datetime
import json
import re
import sys
import time
import urllib.request

from .poc_import import LoopbackRpc

KARMA = {"none": 0, "low": 1, "medium": 2, "high": 3, "very high": 4, "guru": 5}


def parse_profile(html):
    """Join time, karma level and account type from a Wikidot profile page."""
    text = " ".join(re.sub(r"<[^>]+>", " ", html).split())
    since = re.search(r"user since:\s*</dt>\s*<dd>\s*<span class=\"odate time_(\d+)", html)
    karma = re.search(r"Karma level:\s*(none|low|medium|high|very high|guru)\b", text)
    account = re.search(r"Account type:\s*(\w+)", text)
    user_id = re.search(r"USERINFO\.userId = (\d+);", html)
    if not (since and karma and account and user_id):
        raise ValueError("unrecognized Wikidot profile page")
    return {
        "user_id": int(user_id.group(1)),
        "created_at": int(since.group(1)),
        "karma": KARMA[karma.group(1)],
        "is_pro": account.group(1) != "free",
    }


def authors(changes):
    """Distinct revision authors as {user_id: (slug, name)}."""
    found = {}
    for row in changes:
        if row["user_id"] is not None:
            found.setdefault(row["user_id"], (row["user_slug"], row["user_name"]))
    return found


def rfc3339(epoch):
    return datetime.datetime.fromtimestamp(epoch, datetime.UTC).isoformat().replace("+00:00", "Z")


def main(argv):
    changes_path, endpoint, site_id, password_file = argv
    site_id = int(site_id)
    password = open(password_file).read().strip()
    login = LoopbackRpc(endpoint, "", site_id).rpc("login", {
        "name_or_email": "cobalt-import", "password": password,
        "ip_address": "127.0.0.1", "user_agent": "cobalt-wikidot-users",
    })
    rpc = LoopbackRpc(endpoint, login["session_token"], site_id)
    importer = rpc.rpc("session_get", [login["session_token"]])["user_id"]
    for user_id, (slug, name) in sorted(authors(json.load(open(changes_path))).items()):
        with urllib.request.urlopen(f"https://www.wikidot.com/user:info/{slug}", timeout=60) as response:
            profile = parse_profile(response.read().decode())
        if profile["user_id"] != user_id:
            raise ValueError(f"profile {slug} belongs to user {profile['user_id']}, not {user_id}")
        rpc.rpc("import_wikidot_user", {
            "user_id": user_id,
            "created_at": rfc3339(profile["created_at"]),
            "fetched_at": rfc3339(int(time.time())),
            "user_type": "extant", "name": name, "slug": slug,
            "avatar_uploaded_blob_id": None,
            "real_name": None, "gender": None, "birthday": None, "location": None,
            "biography": None, "website": None,
            "karma": profile["karma"], "is_pro": profile["is_pro"],
            "importing_user_id": importer, "ip_address": "127.0.0.1",
        })
        print(f"{user_id} {slug}: imported")
        time.sleep(1)


if __name__ == "__main__":
    main(sys.argv[1:])
