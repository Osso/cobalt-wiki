"""Inventory native Wikidot backups without extracting or exposing page contents."""

import argparse
import codecs
import gzip
import hashlib
import json
import os
from pathlib import Path
import tarfile
import tempfile
import zlib


_CHUNK_SIZE = 64 * 1024
_BLOCK_SIZE = 512
_MALFORMED = "malformed or truncated gzip/tar archive"


class ArchiveError(ValueError):
    """The archive cannot be faithfully inventoried or its manifest written."""


def _validate_path(name: str) -> None:
    if not name or name.startswith("/") or ".." in name.split("/"):
        raise ArchiveError(f"unsafe archive path: {name!r}")


def _classify_file(name: str) -> str:
    root, separator, relative = name.partition("/")
    if not separator or not relative or root not in ("source", "files"):
        raise ArchiveError(f"unsupported regular-file root: {name!r}")
    return "page_source" if root == "source" else "attachment"


def _hash_member(archive: tarfile.TarFile, member: tarfile.TarInfo, role: str) -> str:
    digest = hashlib.sha256()
    decoder = codecs.getincrementaldecoder("utf-8")() if role == "page_source" else None
    extracted = archive.extractfile(member)
    if extracted is None:
        raise ArchiveError(f"cannot read regular file: {member.name!r}")
    size = 0
    try:
        with extracted:
            while chunk := extracted.read(_CHUNK_SIZE):
                size += len(chunk)
                digest.update(chunk)
                if decoder is not None:
                    decoder.decode(chunk)
            if decoder is not None:
                decoder.decode(b"", final=True)
    except UnicodeDecodeError:
        raise ArchiveError(f"source is not valid UTF-8: {member.name!r}") from None
    if size != member.size:
        raise ArchiveError(f"{_MALFORMED}: {member.name!r}")
    return digest.hexdigest()


def _validate_tar_end(stream: gzip.GzipFile, offset: int) -> None:
    # tarfile accepts missing terminators and can stop at a corrupt later header.
    # Read the entire tail to validate both TAR end blocks and the gzip trailer.
    stream.seek(offset)
    padding = 0
    while chunk := stream.read(_CHUNK_SIZE):
        if any(chunk):
            raise ArchiveError(_MALFORMED)
        padding += len(chunk)
    if padding < 2 * _BLOCK_SIZE or padding % _BLOCK_SIZE:
        raise ArchiveError(_MALFORMED)


def _inventory_files(stream: gzip.GzipFile) -> list[dict]:
    files = []
    seen = set()
    end_offset = 0
    with tarfile.open(
        fileobj=stream, mode="r:", encoding="utf-8", errors="strict"
    ) as archive:
        for member in archive:
            _validate_path(member.name)
            if member.size < 0:
                raise ArchiveError(f"{_MALFORMED}: {member.name!r}")
            blocks = (member.size + _BLOCK_SIZE - 1) // _BLOCK_SIZE
            end_offset = member.offset_data + blocks * _BLOCK_SIZE
            if member.issym() or member.islnk():
                raise ArchiveError(f"links are not supported: {member.name!r}")
            if member.isdir():
                continue
            if not member.isfile():
                raise ArchiveError(f"unsupported archive member type: {member.name!r}")
            if member.name in seen:
                raise ArchiveError(f"duplicate file path: {member.name!r}")
            seen.add(member.name)
            role = _classify_file(member.name)
            files.append(
                {
                    "path": member.name,
                    "role": role,
                    "size": member.size,
                    "sha256": _hash_member(archive, member, role),
                }
            )
        _validate_tar_end(stream, end_offset)
    return sorted(files, key=lambda item: item["path"])


def inventory_archive(archive_path: str | os.PathLike) -> dict:
    """Return content-free hashes and role totals, retaining exact archive names."""
    archive_path = Path(archive_path)
    try:
        with archive_path.open("rb") as compressed:
            archive_sha256 = hashlib.file_digest(compressed, "sha256").hexdigest()
            compressed.seek(0)
            with gzip.GzipFile(fileobj=compressed, mode="rb") as stream:
                files = _inventory_files(stream)
    except (
        tarfile.TarError,
        gzip.BadGzipFile,
        EOFError,
        zlib.error,
        UnicodeDecodeError,
    ):
        raise ArchiveError(f"{_MALFORMED}: {archive_path}") from None
    except OSError as error:
        raise ArchiveError(
            f"cannot read archive {archive_path}: {error.strerror}"
        ) from None

    summary = {
        "page_source": {"count": 0, "total_bytes": 0},
        "attachment": {"count": 0, "total_bytes": 0},
    }
    for entry in files:
        totals = summary[entry["role"]]
        totals["count"] += 1
        totals["total_bytes"] += entry["size"]
    return {"archive_sha256": archive_sha256, "summary": summary, "files": files}


def write_manifest(manifest: dict, output_path: str | os.PathLike) -> None:
    """Atomically replace a manifest with an owner-only file in its existing directory."""
    output_path = Path(output_path)
    temporary_path = None
    try:
        descriptor, name = tempfile.mkstemp(
            prefix=f".{output_path.name}.", suffix=".tmp", dir=output_path.parent
        )
        temporary_path = Path(name)
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as output:
            os.fchmod(output.fileno(), 0o600)
            json.dump(manifest, output, ensure_ascii=True, sort_keys=True, indent=2)
            output.write("\n")
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary_path, output_path)
    except OSError as error:
        raise ArchiveError(
            f"cannot write manifest {output_path}: {error.strerror}"
        ) from None
    finally:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="cobalt-backup-inventory",
        description="Inventory a native Wikidot .tar.gz without extracting page contents.",
    )
    parser.add_argument("archive", type=Path)
    parser.add_argument(
        "--output",
        required=True,
        type=Path,
        help="Manifest in an existing protected directory",
    )
    arguments = parser.parse_args(argv)
    try:
        if arguments.archive.resolve() == arguments.output.resolve():
            raise ArchiveError("output must not replace the input archive")
        manifest = inventory_archive(arguments.archive)
        write_manifest(manifest, arguments.output)
    except ArchiveError as error:
        parser.exit(1, f"{parser.prog}: {error}\n")
    return 0
