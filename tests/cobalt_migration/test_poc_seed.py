"""Behavioral contract for private, content-free production seed generation."""

import importlib
import json
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest


class PocSeedTests(unittest.TestCase):
    def setUp(self):
        try:
            self.seed = importlib.import_module("tools.cobalt_migration.poc_seed")
        except ModuleNotFoundError:
            self.fail("POC seed generator is not implemented")
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.password = self.root / "password"
        self.password.write_text("test-only-secret-73910284\n")
        self.password.chmod(0o600)
        self.output = self.root / "seed"

    def generate(self, **changes):
        arguments = dict(
            output=self.output,
            admin_password_file=self.password,
            origin="https://cobalt-company.sakuin.org",
            slug="cobalt-company",
            default_page="home:start",
        )
        arguments.update(changes)
        self.seed.generate_seed(**arguments)

    def load(self, name):
        return json.loads((self.output / f"{name}.json").read_text())

    def test_native_subdomain_site_without_custom_domain_registration(self):
        self.generate()
        self.assertEqual(
            self.load("sites"),
            [
                {
                    "slug": "cobalt-company",
                    "name": "Cobalt Company",
                    "tagline": "",
                    "description": "Access-restricted Cobalt Company proof of concept; source attribution and permissions remain under reconciliation.",
                    "aliases": [],
                    "domains": [],
                    "default-page": "home:start",
                    "layout": "wikidot",
                    "license": "cc-by-sa-3.0",
                    "locale": "en",
                }
            ],
        )
        self.assertEqual(self.load("pages"), {})
        self.assertEqual(self.load("files"), {})
        self.assertEqual(self.load("filters"), [])

    def test_only_technical_admin_can_log_in_and_system_ids_remain(self):
        self.generate()
        users = self.load("users")
        self.assertEqual([user["id"] for user in users], [-1, -2, -3, -4, -5])
        admin, *system = users
        self.assertEqual(admin["name"], "Cobalt Import")
        self.assertEqual(admin["slug"], "cobalt-import")
        self.assertEqual(admin["password"], "test-only-secret-73910284")
        self.assertEqual(admin["type"], "regular")
        self.assertIn("not source authorship", admin["biography"])
        self.assertEqual(admin["email"], "cobalt-import@cobalt-company.sakuin.org")
        self.assertTrue(
            all(
                user["type"] == "system" and user["password"] is None for user in system
            )
        )
        self.assertTrue(all(user["aliases"] == [] for user in users))
        self.assertTrue(
            all("real-name" in user and "user-page" in user for user in users)
        )

    def test_stock_role_permissions_preserved_with_actual_parser_fields(self):
        self.generate()
        roles = self.load("roles")
        stock = json.loads(Path("deepwell/seeder/roles.json").read_text())
        self.assertEqual(len(roles), len(stock))
        for role, original in zip(roles, stock):
            self.assertEqual(role["name"], original["name"])
            self.assertEqual(role["permissions"], original["permissions"])
            self.assertEqual(role["is-virtual"], original["is-virtual"])
            self.assertEqual(role["parent-role"], original.get("parent-role"))
            self.assertEqual(
                set(role),
                {"name", "description", "is-virtual", "parent-role", "permissions"},
            )
        self.assertIn("admin", [role["name"] for role in roles])

    def test_directory_and_every_runtime_file_are_private(self):
        self.generate()
        self.assertEqual(stat.S_IMODE(self.output.stat().st_mode), 0o700)
        self.assertEqual(
            {p.name for p in self.output.iterdir()},
            {
                f"{n}.json"
                for n in ["users", "sites", "pages", "files", "filters", "roles"]
            },
        )
        for path in self.output.iterdir():
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)

    def test_existing_output_refused_without_modification(self):
        self.generate()
        before = {p.name: p.read_bytes() for p in self.output.iterdir()}
        with self.assertRaisesRegex(self.seed.SeedGenerationError, "already exists"):
            self.generate()
        self.assertEqual(
            before, {p.name: p.read_bytes() for p in self.output.iterdir()}
        )

    def test_existing_empty_directory_and_symlink_refused(self):
        self.output.mkdir()
        with self.assertRaises(self.seed.SeedGenerationError):
            self.generate()
        self.output.rmdir()
        self.output.symlink_to(self.root / "missing", target_is_directory=True)
        with self.assertRaises(self.seed.SeedGenerationError):
            self.generate()
        self.assertFalse((self.root / "missing").exists())

    def test_password_file_must_be_private_regular_file(self):
        self.password.chmod(0o644)
        with self.assertRaisesRegex(self.seed.SeedGenerationError, "0600"):
            self.generate()
        self.assertFalse(self.output.exists())
        self.password.chmod(0o600)
        link = self.root / "password-link"
        link.symlink_to(self.password)
        with self.assertRaises(self.seed.SeedGenerationError):
            self.generate(admin_password_file=link)
        self.assertFalse(self.output.exists())

    def test_empty_multiline_and_known_demo_password_refused(self):
        for value in [
            "",
            "wikijumpadmin1",
            "guestuser1",
            "long-first-line-123456\nsecond-line",
        ]:
            with self.subTest(password_kind="invalid"):
                self.password.write_text(value)
                with self.assertRaises(self.seed.SeedGenerationError) as caught:
                    self.generate()
                if value:
                    self.assertNotIn(value, str(caught.exception))
                self.assertFalse(self.output.exists())

    def test_cli_requires_explicit_default_page(self):
        result = subprocess.run(
            [
                sys.executable,
                "-B",
                "-m",
                "tools.cobalt_migration.poc_seed",
                "--output",
                str(self.output),
                "--admin-password-file",
                str(self.password),
                "--origin",
                "https://cobalt-company.sakuin.org",
                "--slug",
                "cobalt-company",
            ],
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("--default-page", result.stderr)
        self.assertFalse(self.output.exists())

    def test_cli_generates_seed_without_disclosing_password(self):
        result = subprocess.run(
            [
                sys.executable,
                "-B",
                "-m",
                "tools.cobalt_migration.poc_seed",
                "--output",
                str(self.output),
                "--admin-password-file",
                str(self.password),
                "--origin",
                "https://cobalt-company.sakuin.org",
                "--slug",
                "cobalt-company",
                "--default-page",
                "home:start",
            ],
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, "")
        self.assertNotIn(self.password.read_text().strip(), result.stdout)
        self.assertIn("no database changes", result.stdout)
        self.assertEqual(self.load("sites")[0]["default-page"], "home:start")

    def test_invalid_origin_slug_or_default_page_leaves_no_output(self):
        for changes in [
            {"origin": "http://cobalt-company.sakuin.org"},
            {"origin": "https://user:secret@cobalt-company.sakuin.org"},
            {"origin": "https://cobalt-company.sakuin.org/path"},
            {"origin": "https://cobalt-company.sakuin.org?x=1"},
            {"slug": "Cobalt Company"},
            {"default_page": ""},
            {"default_page": "../home"},
            {"default_page": "home?edit"},
        ]:
            with self.subTest(fields=list(changes)):
                with self.assertRaises(self.seed.SeedGenerationError):
                    self.generate(**changes)
                self.assertFalse(self.output.exists())


if __name__ == "__main__":
    unittest.main()
