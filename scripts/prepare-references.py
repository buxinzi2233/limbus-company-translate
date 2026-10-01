#!/usr/bin/env python3
"""Verify and unpack the bundled public corpus into a new, isolated directory."""

import argparse
import gzip
import hashlib
from pathlib import Path, PurePosixPath
import shutil
import sqlite3
import tempfile
import zipfile


def verify_checksums(directory: Path, checksum_file: Path) -> None:
    for line in checksum_file.read_text(encoding="utf-8").splitlines():
        expected, name = line.split("  ", 1)
        relative = PurePosixPath(name)
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError(f"Unsafe checksum path in {checksum_file}: {name}")
        path = directory / relative
        with path.open("rb") as handle:
            actual = hashlib.file_digest(handle, "sha256").hexdigest()
        if actual != expected:
            raise ValueError(
                f"SHA-256 mismatch for {path}: expected {expected}, got {actual}; "
                "restore the original corpus bundle before retrying"
            )


def verify_database(path: Path) -> None:
    connection = sqlite3.connect(path.as_uri() + "?mode=ro", uri=True)
    try:
        integrity = connection.execute("PRAGMA integrity_check").fetchall()
        if integrity != [("ok",)]:
            raise sqlite3.DatabaseError(f"Reference integrity check failed for {path}: {integrity}")
        foreign_keys = connection.execute("PRAGMA foreign_key_check").fetchall()
        if foreign_keys:
            raise sqlite3.IntegrityError(f"Reference foreign keys failed for {path}: {foreign_keys}")
        identity = connection.execute(
            "SELECT (SELECT value FROM metadata WHERE key='schema_version')='2' "
            "AND (SELECT value FROM metadata WHERE key='source_host')="
            "'limbuscompany.huijiwiki.com'"
        ).fetchone()
        if identity != (1,):
            raise ValueError(f"Unsupported reference database schema or source host: {path}")
    finally:
        connection.close()


def unpack_chinese(archive: Path, destination: Path) -> None:
    prefix = "LimbusCompany_Data/Lang/LLC_zh-CN/"
    with zipfile.ZipFile(archive) as bundle:
        for entry in bundle.infolist():
            relative = PurePosixPath(entry.filename.removeprefix(prefix))
            if (
                not entry.filename.startswith(prefix)
                or relative.is_absolute()
                or ".." in relative.parts
                or not relative.parts
                or entry.is_dir()
            ):
                raise ValueError(f"Unexpected corpus ZIP member in {archive}: {entry.filename}")
            target = destination / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            with bundle.open(entry) as source, target.open("xb") as output:
                shutil.copyfileobj(source, output)


def prepare(corpus: Path, destination: Path) -> None:
    if destination.exists():
        raise FileExistsError(f"Refusing to replace {destination}; choose a new output directory")
    verify_checksums(corpus, corpus / "SHA256SUMS")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".lct-reference-", dir=destination.parent) as temporary:
        prepared = Path(temporary) / "references"
        prepared.mkdir()
        database = prepared / "references.sqlite3"
        with gzip.open(corpus / "references.sqlite3.gz", "rb") as source, database.open("xb") as output:
            shutil.copyfileobj(source, output)
        verify_checksums(prepared, corpus / "references.sqlite3.sha256")
        verify_database(database)
        unpack_chinese(corpus / "LimbusLocalize_2026092702.zip", prepared / "zh-CN")
        verify_checksums(prepared / "zh-CN", corpus / "upstream-files.sha256")
        shutil.copyfile(corpus / "upstream-files.sha256", prepared / "upstream-files.sha256")
        shutil.copytree(corpus / "licenses", prepared / "licenses")
        shutil.copyfile(corpus / "manifest.json", prepared / "manifest.json")
        database.chmod(0o444)
        prepared.rename(destination)
    print(f"Read-only reference database: {destination / 'references.sqlite3'}")
    print(f"Chinese reference JSON: {destination / 'zh-CN'}")
    print("No project database, translation memory, review state or draft grant was imported.")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path, help="New directory for the offline reference corpus")
    arguments = parser.parse_args()
    prepare(Path(__file__).resolve().parents[1] / "corpus", arguments.destination.resolve())


if __name__ == "__main__":
    main()
