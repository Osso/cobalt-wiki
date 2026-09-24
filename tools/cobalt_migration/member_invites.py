"""Give imported members their real email and a link to choose a password.

The invites file maps member slug to email address, as CSV with a
`slug,email` header or as a JSON object. It holds personal data, so it must
not be readable by group or others. Per member, the tool sets the account's
email (skipping MailCheck verification, like the import), then creates a
set-password link. Without a flag nothing is written: each planned step is
printed with the addresses masked.

    python -m tools.cobalt_migration.member_invites INVITES ENDPOINT SITE_ID PASSWORD_FILE
    ... --send                     # email each member their link (Mailgun)
    ... --apply --links-out FILE   # no email: write the links to FILE (0600)

Reruns are safe: a member who chose a password is skipped, and so is one
with a live link (an emailed one when sending), unless their email changed.
Members are processed at most one per --interval seconds.
"""

import argparse
import csv
import json
import os
import stat
import sys
import time

from .poc_import import LoopbackRpc

IP_ADDRESS = "127.0.0.1"


def load_invites(path):
    """Returns {slug: email}; refuses files others can read and bad rows."""
    if os.stat(path).st_mode & (stat.S_IRWXG | stat.S_IRWXO):
        raise ValueError(f"{path} must not be accessible by group or others (chmod 600)")
    with open(path, newline="") as file:
        if path.endswith(".json"):
            rows = list(json.load(file).items())
        else:
            rows = [(row["slug"], row["email"]) for row in csv.DictReader(file)]
    invites = {}
    for slug, email in rows:
        slug, email = slug.strip(), email.strip()
        if "@" not in email or email.lower().endswith(".invalid"):
            raise ValueError(f"{slug}: not a deliverable email address")
        if slug in invites:
            raise ValueError(f"{slug}: listed twice")
        invites[slug] = email
    if len(set(map(str.lower, invites.values()))) != len(invites):
        raise ValueError("two members share an email address")
    return invites


def mask(email):
    """Enough to tell addresses apart in a log, not enough to use them."""
    user, _, domain = email.partition("@")
    return f"{user[:1]}***@{domain}"


def plan_member(user, status, email, send):
    """The steps one member still needs: a subset of email, link, send."""
    if user is None or user["user_type"] != "regular":
        raise ValueError("no regular account")
    steps = []
    if user["email"] != email:
        steps.append("email")
    pending = status["pending"]
    if status["password_chosen"]:
        return steps
    if "email" in steps or pending is None or (send and not pending["emailed"]):
        steps.append("link")
        if send:
            steps.append("send")
    return steps


def describe(slug, user, email, status, steps):
    parts = []
    if "email" in steps:
        parts.append(f"email {mask(user['email'])} -> {mask(email)}")
    if "send" in steps:
        parts.append(f"email a set-password link to {mask(email)}")
    elif "link" in steps:
        parts.append("create a set-password link")
    if not steps or steps == ["email"]:
        if status["password_chosen"]:
            parts.append("password already chosen")
        elif status["pending"]:
            emailed = "emailed" if status["pending"]["emailed"] else "not emailed"
            parts.append(f"link pending until {status['pending']['expires_at']} ({emailed})")
    return f"{slug}: {'; '.join(parts)}"


def invite_member(rpc, slug, email, site_id, send, write):
    """Applies what the member still needs when `write`; returns (line, steps, path)."""
    user = rpc.rpc("user_get", {"user": slug})
    status = rpc.rpc("password_token_status", {"user": slug}) if user else None
    try:
        steps = plan_member(user, status, email, send)
    except ValueError as error:
        raise ValueError(f"{slug}: {error}") from None
    line = describe(slug, user, email, status, steps)
    path = None
    if write and "email" in steps:
        rpc.rpc("user_edit", {
            "user": slug, "email": email,
            "bypass_filter": True, "bypass_email_verification": True,
            "ip_address": IP_ADDRESS,
        })
    if write and "link" in steps:
        created = rpc.rpc("password_token_create", {"user": slug, "site_id": site_id, "send_email": send})
        path = created["path"]
    return line, steps, path


def run(rpc, invites, site_id, send, write, links, interval, sleep=time.sleep):
    """Processes members in slug order, yielding a line for each."""
    for slug, email in sorted(invites.items()):
        line, steps, path = invite_member(rpc, slug, email, site_id, send, write)
        if path and not send:
            links[slug] = path
        yield line if write else f"would {line}"
        if write and steps:
            sleep(interval)


def write_links(path, links):
    """The links are credentials: the file is created owner-only."""
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w") as file:
        for slug, link in sorted(links.items()):
            file.write(f"{slug}\t{link}\n")


def parse_args(argv):
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("invites")
    parser.add_argument("endpoint")
    parser.add_argument("site_id", type=int)
    parser.add_argument("password_file")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--send", action="store_true", help="email each member their link")
    mode.add_argument("--apply", action="store_true", help="create links without email")
    parser.add_argument("--links-out", help="new owner-only file for --apply links")
    parser.add_argument("--interval", type=float, default=2.0)
    args = parser.parse_args(argv)
    if args.apply != bool(args.links_out):
        parser.error("--apply and --links-out go together")
    return args


def main(argv):
    args = parse_args(argv)
    invites = load_invites(args.invites)
    with open(args.password_file) as file:
        password = file.read().strip()
    login = LoopbackRpc(args.endpoint, "", args.site_id).rpc("login", {
        "name_or_email": "cobalt-import", "password": password,
        "ip_address": IP_ADDRESS, "user_agent": "cobalt-member-invites",
    })
    rpc = LoopbackRpc(args.endpoint, login["session_token"], args.site_id)
    links = {}
    write = args.send or args.apply
    try:
        for line in run(rpc, invites, args.site_id, args.send, write, links, args.interval):
            print(line, flush=True)
    finally:
        if links:
            write_links(args.links_out, links)
            print(f"{len(links)} links written to {args.links_out}")


if __name__ == "__main__":
    main(sys.argv[1:])
