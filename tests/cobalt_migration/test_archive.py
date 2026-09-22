"""Behavioral tests using synthetic backups only; never read the real export."""

import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


def make_tar(files=(), special_members=()):
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w", format=tarfile.PAX_FORMAT) as archive:
        for name, content in files:
            member = tarfile.TarInfo(name)
            member.size = len(content)
            archive.addfile(member, io.BytesIO(content))
        for member in special_members:
            archive.addfile(member)
    return buffer.getvalue()


def special_member(name, kind, target=""):
    member = tarfile.TarInfo(name)
    member.type = kind
    member.linkname = target
    return member


class ArchiveInventoryTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.directory = Path(directory.name)
        self.archive = self.directory / "backup.tar.gz"
        self.output = self.directory / "manifest.json"

    def write_archive(self, files=(), special_members=()):
        raw_tar = make_tar(files, special_members)
        self.archive.write_bytes(gzip.compress(raw_tar, mtime=0))
        return raw_tar

    def run_cli(self, output=None, archive=None):
        return subprocess.run(
            [
                sys.executable,
                "-m",
                "tools.cobalt_migration",
                str(self.archive if archive is None else archive),
                "--output",
                str(self.output if output is None else output),
            ],
            cwd=ROOT,
            env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"},
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )

    def assert_rejected(self, message):
        previous = b"previous manifest stays intact\n"
        self.output.write_bytes(previous)
        result = self.run_cli()
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn(message, result.stderr)
        self.assertEqual(result.stdout, "")
        self.assertEqual(self.output.read_bytes(), previous)
        self.assertEqual(
            {entry.name for entry in self.directory.iterdir()},
            {"backup.tar.gz", "manifest.json"},
        )
        return result

    def test_exact_paths_roles_sizes_hashes_and_summaries_without_source_contents(self):
        files = [
            ("source/character_hero.txt", b"name: SYNTHETIC_PRIVATE_VALUE\n"),
            ("files/character_hero/portrait.bin", b"\x00\xff\x01\x80portrait"),
            ("source/character__template.txt", b"[[form]]\n[[/form]]\n"),
            ("source/player_caf\u00e9.txt", "pronouns: \u00e9l\n".encode()),
            ("source/empty.txt", b""),
        ]
        directories = [
            special_member("source/", tarfile.DIRTYPE),
            special_member("files/", tarfile.DIRTYPE),
            special_member("files/character_hero/", tarfile.DIRTYPE),
        ]
        self.write_archive(files, directories)
        result = self.run_cli()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "")
        self.assertEqual(result.stderr, "")
        manifest = json.loads(self.output.read_text())
        expected_files = [
            {
                "path": name,
                "role": "page_source" if name.startswith("source/") else "attachment",
                "size": len(content),
                "sha256": hashlib.sha256(content).hexdigest(),
            }
            for name, content in sorted(files)
        ]
        self.assertEqual(
            manifest,
            {
                "archive_sha256": hashlib.sha256(self.archive.read_bytes()).hexdigest(),
                "summary": {
                    "page_source": {
                        "count": 4,
                        "total_bytes": sum(
                            len(content)
                            for name, content in files
                            if name.startswith("source/")
                        ),
                    },
                    "attachment": {"count": 1, "total_bytes": len(files[1][1])},
                },
                "files": expected_files,
            },
        )
        self.assertNotIn("SYNTHETIC_PRIVATE_VALUE", self.output.read_text())
        self.assertEqual(
            {entry.name for entry in self.directory.iterdir()},
            {"backup.tar.gz", "manifest.json"},
        )

    def test_output_is_deterministic_and_independent_of_archive_filesystem_name(self):
        self.write_archive([("source/page.txt", b"Some wikitext\n")])
        first = self.run_cli()
        self.assertEqual(first.returncode, 0, first.stderr)
        original_manifest = self.output.read_bytes()
        renamed = self.directory / "renamed.tar.gz"
        self.archive.rename(renamed)
        second = self.run_cli(archive=renamed)
        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertEqual(self.output.read_bytes(), original_manifest)

    def test_replacement_is_atomic_and_owner_only(self):
        self.write_archive([("source/page.txt", b"New source\n")])
        self.output.write_bytes(b"previous complete manifest")
        self.output.chmod(0o644)
        old_umask = os.umask(0)
        try:
            with self.output.open("rb") as previous_reader:
                result = self.run_cli()
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(previous_reader.read(), b"previous complete manifest")
        finally:
            os.umask(old_umask)
        self.assertEqual(stat.S_IMODE(self.output.stat().st_mode), 0o600)
        self.assertEqual(
            json.loads(self.output.read_text())["summary"]["page_source"]["count"], 1
        )
        self.assertEqual(
            {entry.name for entry in self.directory.iterdir()},
            {"backup.tar.gz", "manifest.json"},
        )

    def test_failed_replacement_cleans_temporary_file(self):
        self.write_archive([("source/page.txt", b"source")])
        self.output.mkdir()
        sentinel = self.output / "keep"
        sentinel.write_bytes(b"existing directory")
        result = self.run_cli()
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("cannot write manifest", result.stderr)
        self.assertEqual(sentinel.read_bytes(), b"existing directory")
        self.assertEqual(
            {entry.name for entry in self.directory.iterdir()},
            {"backup.tar.gz", "manifest.json"},
        )

    def test_output_cannot_replace_input_archive(self):
        self.write_archive([("source/page.txt", b"source")])
        original = self.archive.read_bytes()
        result = self.run_cli(output=self.archive)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("output must not replace the input archive", result.stderr)
        self.assertEqual(self.archive.read_bytes(), original)

    def test_empty_archive_has_zero_totals(self):
        self.write_archive()
        result = self.run_cli()
        self.assertEqual(result.returncode, 0, result.stderr)
        manifest = json.loads(self.output.read_text())
        self.assertEqual(manifest["files"], [])
        self.assertEqual(
            manifest["summary"],
            {
                "page_source": {"count": 0, "total_bytes": 0},
                "attachment": {"count": 0, "total_bytes": 0},
            },
        )

    def test_multibyte_utf8_across_read_boundary(self):
        content = b"a" * (64 * 1024 - 1) + "\u20ac\n".encode()
        self.write_archive([("source/page.txt", content)])
        result = self.run_cli()
        self.assertEqual(result.returncode, 0, result.stderr)
        record = json.loads(self.output.read_text())["files"][0]
        self.assertEqual(record["size"], len(content))
        self.assertEqual(record["sha256"], hashlib.sha256(content).hexdigest())

    def test_invalid_utf8_reports_path_without_private_contents(self):
        for content in (b"SYNTHETIC_PRIVATE_VALUE\xff", b"valid prefix\xe2\x82"):
            with self.subTest(content_length=len(content)):
                self.write_archive([("source/private_page.txt", content)])
                result = self.assert_rejected("source is not valid UTF-8")
                self.assertIn("source/private_page.txt", result.stderr)
                self.assertNotIn("SYNTHETIC_PRIVATE_VALUE", result.stderr)
                self.assertNotIn("valid prefix", result.stderr)

    def test_duplicate_file_paths_are_rejected(self):
        self.write_archive(
            [("source/page.txt", b"first"), ("source/page.txt", b"second")]
        )
        self.assert_rejected("duplicate file path: 'source/page.txt'")

    def test_unsupported_regular_file_roots_are_rejected(self):
        for name in ("other/page.txt", "metadata.json", "source"):
            with self.subTest(name=name):
                self.write_archive([(name, b"content")])
                self.assert_rejected("unsupported regular-file root")

    def test_absolute_and_traversal_paths_are_rejected_in_files_and_directories(self):
        for name in (
            "/source/page.txt",
            "../source/page.txt",
            "source/../private.txt",
            "files/hero/../../private.bin",
        ):
            for directory in (False, True):
                with self.subTest(name=name, directory=directory):
                    if directory:
                        self.write_archive(
                            special_members=[special_member(name, tarfile.DIRTYPE)]
                        )
                    else:
                        self.write_archive([(name, b"content")])
                    self.assert_rejected("unsafe archive path")

    def test_symbolic_and_hard_links_are_rejected(self):
        for kind in (tarfile.SYMTYPE, tarfile.LNKTYPE):
            with self.subTest(kind=kind):
                self.write_archive(
                    [("source/page.txt", b"source")],
                    [special_member("files/page/link", kind, "source/page.txt")],
                )
                self.assert_rejected("links are not supported")

    def test_special_files_are_rejected(self):
        self.write_archive(
            special_members=[special_member("files/page/device", tarfile.CHRTYPE)]
        )
        self.assert_rejected("unsupported archive member type")

    def test_malformed_archive_is_rejected(self):
        for content in (b"not a gzip file", gzip.compress(b"not a tar file", mtime=0)):
            with self.subTest(length=len(content)):
                self.archive.write_bytes(content)
                self.assert_rejected("malformed or truncated gzip/tar archive")

    def test_truncated_gzip_and_invalid_crc_are_rejected(self):
        self.write_archive([("source/page.txt", b"source\n")])
        original = self.archive.read_bytes()
        invalid_crc = bytearray(original)
        invalid_crc[-8] ^= 1
        for content in (
            original[:-8],
            original[: len(original) // 2],
            bytes(invalid_crc),
        ):
            with self.subTest(length=len(content)):
                self.archive.write_bytes(content)
                self.assert_rejected("malformed or truncated gzip/tar archive")

    def test_truncated_tar_payload_or_terminator_is_rejected(self):
        raw_tar = make_tar([("source/page.txt", b"x" * 100)])
        for end in (522, 1024, 1536):
            with self.subTest(end=end):
                self.archive.write_bytes(gzip.compress(raw_tar[:end], mtime=0))
                self.assert_rejected("malformed or truncated gzip/tar archive")

    def test_tar_with_two_end_blocks_needs_no_extra_record_padding(self):
        raw_tar = make_tar([("source/page.txt", b"source")])
        self.archive.write_bytes(gzip.compress(raw_tar[:2048], mtime=0))
        result = self.run_cli()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(self.output.read_text())["files"][0]["size"], 6)

    def test_corrupt_later_header_and_data_after_terminator_are_rejected(self):
        raw_tar = make_tar(
            [("source/first.txt", b"one"), ("source/second.txt", b"two")]
        )
        corrupted = bytearray(raw_tar)
        corrupted[1024] ^= 1
        for content in (bytes(corrupted), raw_tar + b"unexpected trailing data"):
            with self.subTest(length=len(content)):
                self.archive.write_bytes(gzip.compress(content, mtime=0))
                self.assert_rejected("malformed or truncated gzip/tar archive")

    def test_missing_input_reports_read_failure_without_creating_output(self):
        result = self.run_cli()
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("cannot read archive", result.stderr)
        self.assertFalse(self.output.exists())

    def test_missing_output_directory_is_not_created(self):
        self.write_archive([("source/page.txt", b"source")])
        output = self.directory / "not-provisioned" / "manifest.json"
        result = self.run_cli(output=output)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("cannot write manifest", result.stderr)
        self.assertFalse(output.parent.exists())


if __name__ == "__main__":
    unittest.main()
