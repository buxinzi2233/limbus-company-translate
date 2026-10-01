#!/usr/bin/env python3
"""Exercise offline corpus preparation using the real bundled snapshot, without production state."""

import argparse
import hashlib
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import tempfile


def verify(root: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="lct-corpus-check-") as temporary:
        scratch = Path(temporary)
        destination = scratch / "prepared"
        command = [sys.executable, str(root / "scripts/prepare-references.py"), str(destination)]
        subprocess.run(command, check=True)
        database = destination / "references.sqlite3"
        original_digest = hashlib.sha256(database.read_bytes()).hexdigest()
        connection = sqlite3.connect(database.as_uri() + "?mode=ro", uri=True)
        try:
            sources = connection.execute("SELECT path,sha256 FROM sources").fetchall()
            for name, expected in sources:
                relative = name.removeprefix("LimbusCompany_Data/Lang/LLC_zh-CN/")
                actual = hashlib.sha256((destination / "zh-CN" / relative).read_bytes()).hexdigest()
                if actual != expected:
                    raise ValueError(f"Restored source differs from reference database: {name}")
            tables = {row[0] for row in connection.execute("SELECT name FROM sqlite_master WHERE type='table'")}
            if tables.intersection({"projects", "segments", "tm_entries", "review_events", "draft_grants"}):
                raise ValueError("Public reference bundle unexpectedly contains private workflow tables")
            if connection.execute("SELECT count(*) FROM sources WHERE path LIKE '/%'").fetchone() != (0,):
                raise ValueError("Public reference bundle contains machine-local source paths")
            if connection.execute("SELECT count(*) FROM corpus_metadata WHERE value LIKE '/%'").fetchone() != (0,):
                raise ValueError("Public reference metadata contains machine-local paths")
        finally:
            connection.close()
        repeated = subprocess.run(command, capture_output=True, text=True, check=False)
        if repeated.returncode == 0 or "Refusing to replace" not in repeated.stderr:
            raise RuntimeError(f"Existing destination was not rejected: {repeated.stderr}")
        if hashlib.sha256(database.read_bytes()).hexdigest() != original_digest:
            raise ValueError("Repeated preparation changed the existing reference database")
        copy = scratch / "damaged-bundle"
        shutil.copytree(root / "corpus", copy / "corpus")
        (copy / "scripts").mkdir()
        shutil.copyfile(root / "scripts/prepare-references.py", copy / "scripts/prepare-references.py")
        compressed = copy / "corpus/references.sqlite3.gz"
        with compressed.open("r+b") as handle:
            handle.write(b"broken")
        damaged_destination = scratch / "damaged-output"
        damaged = subprocess.run(
            [sys.executable, str(copy / "scripts/prepare-references.py"), str(damaged_destination)],
            capture_output=True, text=True, check=False,
        )
        if damaged.returncode == 0 or "SHA-256 mismatch" not in damaged.stderr:
            raise RuntimeError(f"Corrupt snapshot was not rejected: {damaged.stderr}")
        if damaged_destination.exists():
            raise ValueError("Corrupt snapshot left a published destination")
        print(f"PASS: restored and hashed {len(sources)} original JSON files; no private workflow tables")
        print("PASS: existing destination unchanged; corrupt snapshot rejected without publishing output")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="Source checkout containing corpus/ and scripts/")
    arguments = parser.parse_args()
    verify(arguments.source.resolve())


if __name__ == "__main__":
    main()
