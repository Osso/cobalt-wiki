"""Generate private, content-free seed input; never connect to the target."""

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import stat
from urllib.parse import urlsplit


class SeedGenerationError(ValueError):
    """Seed generation cannot proceed without risking incorrect provisioning."""


_SEED_SOURCE = Path(__file__).resolve().parents[2] / "deepwell" / "seeder"
_SYSTEM_IDS = (-2, -3, -4, -5)


def _validate_site(origin, slug, default_page):
    try:
        parts = urlsplit(origin)
        valid_origin = (
            parts.scheme == "https"
            and parts.hostname
            and parts.netloc == parts.hostname
            and not parts.path
            and not parts.query
            and not parts.fragment
            and re.fullmatch(r"[a-z0-9]+(?:[a-z0-9.-]*[a-z0-9])?", parts.hostname)
        )
    except ValueError:
        valid_origin = False
    if not valid_origin:
        raise SeedGenerationError("origin must be a canonical HTTPS hostname only")
    if not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", slug):
        raise SeedGenerationError("slug must be a canonical lower-case site slug")
    if not re.fullmatch(r"[a-z0-9_-]+(?::[a-z0-9_-]+)?", default_page):
        raise SeedGenerationError(
            "default-page must be an explicit canonical page name"
        )
    return parts.hostname


def _read_password(path, stock_users):
    try:
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    except OSError:
        raise SeedGenerationError("cannot open admin password file safely") from None
    with os.fdopen(fd, "r", encoding="utf-8") as stream:
        info = os.fstat(stream.fileno())
        if (
            not stat.S_ISREG(info.st_mode)
            or info.st_uid != os.getuid()
            or stat.S_IMODE(info.st_mode) != 0o600
        ):
            raise SeedGenerationError(
                "admin password file must be owner-owned and 0600"
            )
        try:
            password = stream.read().removesuffix("\n")
        except UnicodeError:
            raise SeedGenerationError(
                "admin password file must contain UTF-8"
            ) from None
    known = {user["password"] for user in stock_users if user.get("password")}
    if (
        len(password) < 16
        or password in known
        or not password.strip()
        or any(ord(char) < 32 or ord(char) == 127 for char in password)
    ):
        raise SeedGenerationError(
            "admin password must be a non-demo single-line secret of at least 16 characters"
        )
    return password


def _build_users(stock, password, domain):
    users = [
        {
            "id": -1,
            "type": "regular",
            "name": "Cobalt Import",
            "slug": "cobalt-import",
            "email": f"cobalt-import@{domain}",
            "password": password,
            "locales": ["en"],
            "real-name": "Cobalt Import",
            "gender": None,
            "birthday": None,
            "location": None,
            "biography": "Technical POC administrator and import principal; attribution to this account records migration, not source authorship.",
            "user-page": None,
            "aliases": [],
        }
    ]
    for user_id in _SYSTEM_IDS:
        matches = [user for user in stock if user["id"] == user_id]
        if len(matches) != 1 or matches[0]["type"] != "system":
            raise SeedGenerationError("stock system-user contract changed")
        original = matches[0]
        users.append(
            {
                "id": user_id,
                "type": "system",
                "name": original["name"],
                "slug": original["slug"],
                "email": original["email"],
                "password": None,
                "locales": original["locales"],
                "real-name": original.get("real-name"),
                "gender": original.get("gender"),
                "birthday": original.get("birthday"),
                "location": original.get("location"),
                "biography": original.get("biography"),
                "user-page": original.get("user-page"),
                "aliases": [],
            }
        )
    return users


def _build_documents(domain, slug, default_page, password, stock_users, stock_roles):
    # Match SeedData::Role's kebab-case contract. Stock parent_role/is-system
    # keys are ignored by that parser; do not accidentally enable inheritance.
    roles = [
        {
            "name": role["name"],
            "description": role["description"],
            "is-virtual": role["is-virtual"],
            "parent-role": role.get("parent-role"),
            "permissions": role["permissions"],
        }
        for role in stock_roles
    ]
    site = {
        "slug": slug,
        "name": "Cobalt Company",
        "tagline": "",
        "description": "Access-restricted Cobalt Company proof of concept; source attribution and permissions remain under reconciliation.",
        "aliases": [],
        "domains": [{"domain": domain, "www-redirect": False}],
        "preferred-domain": domain,
        "default-page": default_page,
        "layout": "wikidot",
        "license": "cc-by-sa-3.0",
        "locale": "en",
    }
    return {
        "users": _build_users(stock_users, password, domain),
        "sites": [site],
        "pages": {},
        "files": {},
        "filters": [],
        "roles": roles,
    }


def _write_seed(output, documents):
    try:
        output.mkdir(mode=0o700)
    except FileExistsError:
        raise SeedGenerationError(
            "seed output already exists; refusing to replace it"
        ) from None
    try:
        output.chmod(0o700)
        for name, document in documents.items():
            path = output / f"{name}.json"
            fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(fd, "w", encoding="utf-8") as stream:
                os.fchmod(stream.fileno(), 0o600)
                json.dump(document, stream, ensure_ascii=False, indent=2)
                stream.write("\n")
                stream.flush()
                os.fsync(stream.fileno())
    except BaseException:
        shutil.rmtree(output)
        raise


def generate_seed(*, output, admin_password_file, origin, slug, default_page):
    """Write six SeedData JSON files to a new 0700 directory.

    The caller must verify default_page against the acquired canonical listing.
    This generates no page content, configuration file, or provisioning marker.
    """
    domain = _validate_site(origin, slug, default_page)
    stock_users = json.loads((_SEED_SOURCE / "users.json").read_text())
    stock_roles = json.loads((_SEED_SOURCE / "roles.json").read_text())
    password = _read_password(admin_password_file, stock_users)
    documents = _build_documents(
        domain, slug, default_page, password, stock_users, stock_roles
    )
    _write_seed(Path(output), documents)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--admin-password-file", type=Path, required=True)
    parser.add_argument("--origin", required=True)
    parser.add_argument("--slug", required=True)
    parser.add_argument("--default-page", required=True)
    args = parser.parse_args(argv)
    try:
        generate_seed(**vars(args))
    except (SeedGenerationError, OSError, ValueError) as error:
        parser.exit(1, f"Seed generation failed: {error}\n")
    print("Generated private seed directory; no database changes performed.")


if __name__ == "__main__":
    main()
