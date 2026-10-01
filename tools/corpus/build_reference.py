"""Build a versioned monolingual corpus beside an existing attributed Wiki schema.

This does not read LCT drafts, create bilingual pairs, or grant human approval.
Every indexed Chinese string retains its source file hash and JSON pointer.
"""

import argparse
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import re
import sqlite3
from typing import Iterator


type JsonValue = None | bool | int | float | str | list[JsonValue] | dict[str, JsonValue]
HAN = re.compile(r"[\u3400-\u4dbf\u4e00-\u9fff]")
SCHEMA = """
CREATE TABLE sources(id INTEGER PRIMARY KEY,path TEXT NOT NULL UNIQUE,
 origin TEXT NOT NULL,kind TEXT NOT NULL,sha256 TEXT NOT NULL) STRICT;
CREATE TABLE documents(source_id INTEGER PRIMARY KEY REFERENCES sources(id),body TEXT NOT NULL) STRICT;
CREATE TABLE corpus(id INTEGER PRIMARY KEY,source_id INTEGER NOT NULL REFERENCES sources(id),
 pointer TEXT NOT NULL,field TEXT NOT NULL,record_id TEXT NOT NULL,text_zh TEXT NOT NULL,
 UNIQUE(source_id,pointer)) STRICT;
CREATE TABLE terms(id INTEGER PRIMARY KEY,source_id INTEGER NOT NULL REFERENCES sources(id),
 pointer TEXT NOT NULL,record_id TEXT NOT NULL,name_zh TEXT NOT NULL,description TEXT NOT NULL,
 UNIQUE(source_id,pointer)) STRICT;
CREATE TABLE corpus_metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL) STRICT;
CREATE VIRTUAL TABLE corpus_fts USING fts5(text_zh,content='corpus',content_rowid='id',tokenize='trigram');
CREATE INDEX corpus_source ON corpus(source_id);
CREATE INDEX terms_name ON terms(name_zh);
"""


@dataclass(frozen=True)
class TextField:
    pointer: str
    field: str
    record_id: str
    text_zh: str
    description: str


def pointer_key(key: str) -> str:
    return key.replace("~", "~0").replace("/", "~1")


def fields(value: JsonValue, pointer: str, field: str, identity: str, description: str) -> Iterator[TextField]:
    if isinstance(value, str):
        if HAN.search(value):
            yield TextField(pointer, field, identity, value, description)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield from fields(child, f"{pointer}/{index}", field, identity, description)
    elif isinstance(value, dict):
        ids = [(key, value[key]) for key in ("id", "key", "index") if key in value]
        own_identity = json.dumps(ids[0], ensure_ascii=False) if ids else identity
        descriptions = [value[key] for key in ("desc", "description", "content") if isinstance(value.get(key), str)]
        own_description = "\n".join(str(item) for item in descriptions)
        for key, child in value.items():
            yield from fields(child, f"{pointer}/{pointer_key(key)}", key, own_identity, own_description)


def build(database: Path, source: Path, origin: str, version: str, manifest: Path) -> dict[str, int | str]:
    if not database.is_file():
        raise FileNotFoundError(f"Initialize the attributed Wiki schema first: {database}")
    expected = json.loads(manifest.read_text(encoding="utf-8"))["files"]
    if not isinstance(expected, dict) or not all(isinstance(k, str) and isinstance(v, str) for k, v in expected.items()):
        raise ValueError(f"Invalid file hash mapping in manifest: {manifest}")
    actual_files = {p.relative_to(source).as_posix(): p for p in source.rglob("*") if p.is_file()}
    if set(actual_files) != set(expected):
        raise ValueError(f"Source membership differs from verified manifest: {source}")
    for relative, path in actual_files.items():
        if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest() != expected[relative]:
            raise ValueError(f"Source file changed or is a symlink: {path}")
    with sqlite3.connect(database) as conn:
        conn.execute("PRAGMA foreign_keys=ON")
        conn.executescript(SCHEMA)
        for relative, path in sorted(actual_files.items()):
            if path.suffix.lower() != ".json":
                continue
            value: JsonValue = json.loads(path.read_text(encoding="utf-8-sig"))
            cursor = conn.execute("INSERT INTO sources(path,origin,kind,sha256) VALUES(?,?,?,?)", (str(path.resolve()), origin, "public_zeroasso_chinese_reference", expected[relative]))
            source_id = cursor.lastrowid
            for row in fields(value, "", "", "", ""):
                conn.execute("INSERT INTO corpus(source_id,pointer,field,record_id,text_zh) VALUES(?,?,?,?,?)", (source_id,row.pointer,row.field,row.record_id,row.text_zh))
                if row.field in {"name", "title"}:
                    conn.execute("INSERT INTO terms(source_id,pointer,record_id,name_zh,description) VALUES(?,?,?,?,?)", (source_id,row.pointer,row.record_id,row.text_zh,row.description))
        license_path = source / "Info/LICENSE"
        license_hash = hashlib.sha256(license_path.read_bytes()).hexdigest()
        conn.executemany("INSERT INTO corpus_metadata VALUES(?,?)", [
            ("version", version), ("origin", origin), ("license_file", str(license_path.resolve())),
            ("license_sha256", license_hash), ("purpose", "Chinese reference only; not aligned Korean-Chinese TM or human-approved translation"),
            ("input_manifest", str(manifest.resolve())), ("old_drafts_imported", "0"),
            ("modifications", "Extracted Han-containing strings with original JSON pointers; indexed name/title occurrences; source bytes unchanged"),
        ])
        conn.execute("INSERT INTO corpus_fts(corpus_fts) VALUES('rebuild')")
        conn.execute("INSERT INTO corpus_fts(corpus_fts,rank) VALUES('integrity-check',1)")
        if conn.execute("PRAGMA integrity_check").fetchone()[0] != "ok" or conn.execute("PRAGMA foreign_key_check").fetchall():
            raise ValueError(f"Reference database integrity check failed: {database}")
        return {"database": str(database.resolve()), "json_files": conn.execute("SELECT count(*) FROM sources").fetchone()[0], "chinese_fields": conn.execute("SELECT count(*) FROM corpus").fetchone()[0], "name_occurrences": conn.execute("SELECT count(*) FROM terms").fetchone()[0]}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--database", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--origin", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(build(args.database,args.source,args.origin,args.version,args.manifest), ensure_ascii=False))


if __name__ == "__main__":
    main()
