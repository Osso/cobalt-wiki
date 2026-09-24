"""Create native Cobalt accounts for the Wikidot site members.

Each member becomes a Deepwell user with the Wikidot user ID, name and slug
(activated from its wikidot_user record, imported first from the public
profile when the member authored no revision, keeping its Wikidot account
creation time), a site membership dated at the site join time, and site
roles: member for everyone, plus admin or moderator, plus root for the master
admin. Accounts get an unreachable placeholder email and a random password
that is never stored or shown, so nobody can log in until a set-password link
is sent. Existing accounts are left untouched and reported, so reruns are safe.

    python -m tools.cobalt_migration.wikidot_members MEMBERS_JSON \\
        http://127.0.0.1:27471/jsonrpc 6000000 PASSWORD_FILE
"""

import json
import secrets
import sys
import time
import urllib.request

from .poc_import import LoopbackRpc
from .wikidot_users import parse_profile, rfc3339

IP_ADDRESS = "127.0.0.1"


def placeholder_email(user_id):
    """RFC 2606 reserves .invalid, so this address can never receive mail."""
    return f"wikidot-{user_id}@members.invalid"


def wanted_roles(member):
    roles = {"member"}
    if member["role"] in ("admin", "moderator"):
        roles.add(member["role"])
    if member["master_admin"]:
        roles.add("root")
    return roles


def fetch_profile(slug):
    with urllib.request.urlopen(f"https://www.wikidot.com/user:info/{slug}", timeout=60) as response:
        html = response.read().decode()
    time.sleep(1)
    return parse_profile(html)


def import_wikidot_record(rpc, member, importer, fetch):
    profile = fetch(member["slug"])
    if profile["user_id"] != member["user_id"]:
        raise ValueError(f"profile {member['slug']} belongs to user {profile['user_id']}, not {member['user_id']}")
    rpc.rpc("import_wikidot_user", {
        "user_id": member["user_id"],
        "created_at": rfc3339(profile["created_at"]),
        "fetched_at": rfc3339(int(time.time())),
        "user_type": "extant", "name": member["name"], "slug": member["slug"],
        "avatar_uploaded_blob_id": None,
        "real_name": None, "gender": None, "birthday": None, "location": None,
        "biography": None, "website": None,
        "karma": profile["karma"], "is_pro": profile["is_pro"],
        "importing_user_id": importer, "ip_address": IP_ADDRESS,
    })


def import_member(rpc, member, site_id, role_ids, importer, fetch=fetch_profile):
    """Create one member's account; returns what happened, never a secret."""
    user_id = member["user_id"]
    found = rpc.rpc("user_get", {"user": user_id})
    if found is None:
        import_wikidot_record(rpc, member, importer, fetch)
    elif found["user_type"] != "wikidot":
        return "exists"
    elif found["slug"] != member["slug"]:
        raise ValueError(f"wikidot user {user_id} has slug {found['slug']}, not {member['slug']}")

    # Membership and roles go first: they only need the wikidot_user record, and
    # the account itself, created last, marks the member as done for reruns.
    if rpc.rpc("member_get", {"site_id": site_id, "user_id": user_id}) is None:
        rpc.rpc("member_set", {
            "site_id": site_id, "user_id": user_id,
            "metadata": {"accepted": {"cause": "accepted", "user_id": importer}},
            "created_by": importer,
            "joined_at": rfc3339(member["member_since_epoch"]),
            "ip_address": IP_ADDRESS,
        })
    held = {role["name"] for role in rpc.rpc("user_role_list", {"site_id": site_id, "user_id": user_id})}
    for name in sorted(wanted_roles(member) - held):
        rpc.rpc("user_role_grant", {
            "user_id": user_id, "role_id": role_ids[name], "site_id": site_id,
            "assigning_user_id": importer, "expires_at": None, "ip_address": IP_ADDRESS,
        })

    rpc.rpc("user_activate_from_wikidot", {
        "user_id": user_id, "user_type": "regular",
        "email": placeholder_email(user_id), "locales": ["en"],
        "password": secrets.token_urlsafe(48),
        "bypass_filter": True, "bypass_email_verification": True,
        "ip_address": IP_ADDRESS,
    })
    return "created"


def main(argv):
    members_path, endpoint, site_id, password_file = argv
    site_id = int(site_id)
    password = open(password_file).read().strip()
    login = LoopbackRpc(endpoint, "", site_id).rpc("login", {
        "name_or_email": "cobalt-import", "password": password,
        "ip_address": IP_ADDRESS, "user_agent": "cobalt-wikidot-members",
    })
    rpc = LoopbackRpc(endpoint, login["session_token"], site_id)
    importer = rpc.rpc("session_get", [login["session_token"]])["user_id"]
    role_ids = {role["name"]: role["role_id"] for role in rpc.rpc("role_list", {"site_id": site_id})}
    missing = {"root", "admin", "moderator", "member"} - role_ids.keys()
    if missing:
        raise ValueError(f"site {site_id} lacks roles {sorted(missing)}")
    for member in sorted(json.load(open(members_path))["members"], key=lambda m: m["user_id"]):
        outcome = import_member(rpc, member, site_id, role_ids, importer)
        print(f"{member['user_id']} {member['slug']}: {outcome}")


if __name__ == "__main__":
    main(sys.argv[1:])
