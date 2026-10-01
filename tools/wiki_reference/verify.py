"""Exercise real saved captures against temporary SQLite and verify the delivered library."""

import argparse
import copy
import hashlib
import json
from pathlib import Path
import sqlite3
from tempfile import TemporaryDirectory

from model import CaptureError, parse_capture
from store import groups, import_capture, initialize, search, status


def verify(database: Path, captures: Path) -> dict[str, object]:
    payloads = [path.read_text(encoding="utf-8") for path in sorted(captures.glob("*.json"))]
    if not payloads:
        raise ValueError(f"No real API captures found in {captures}")
    pages = [parse_capture(payload) for payload in payloads]
    before = hashlib.sha256(database.read_bytes()).hexdigest()
    with TemporaryDirectory(prefix="lct-wiki-test-") as folder:
        root = Path(folder)
        db = root / "test.sqlite3"
        archive = root / "captures"
        initialize(db)
        for payload in sorted(payloads, key=lambda value: parse_capture(value).revision_id):
            import_capture(db, archive, payload)
            assert import_capture(db, archive, payload)["status"] == "unchanged"
        baseline = status(db)
        assert not search(db, "路线A-RPG导航", 10)["passages"]
        for query in ("李", "李箱", "呼吸法"):
            found = search(db, query, 10)
            assert found["terms"] and found["passages"], query
            for result in found["terms"] + found["passages"]:
                assert all(result[key] for key in ("revision_id", "license_url", "attribution", "history_url"))
        assert not search(db, "正在加载GlobalSearch", 10)["passages"]
        assert not search(db, "%", 10)["terms"]
        assert all(len(part) <= 1200 for page in pages for part in groups(page.paragraphs))
        term_results = search(db, "呼吸法", 50)["terms"]
        assert all("呼吸法" in term["context"] for term in term_results if term["name_zh"] == "呼吸法")
        malformed = []
        source = next(json.loads(item) for item in payloads if json.loads(item).get("kind") != "dante_note_widget")
        wrong_host = copy.deepcopy(source)
        wrong_host["query_url"] = "https://other.huijiwiki.com/api.php?action=query"
        malformed.append(wrong_host)
        wrong_revision = copy.deepcopy(source)
        wrong_revision["render"]["parse"]["revid"] += 1
        malformed.append(wrong_revision)
        missing_source = copy.deepcopy(source)
        del missing_source["page"]["revisions"]
        malformed.append(missing_source)
        for item in malformed:
            try:
                import_capture(db, archive, json.dumps(item, ensure_ascii=False))
            except CaptureError:
                pass
            else:
                raise AssertionError("Malformed external capture was accepted")
            assert status(db) == baseline
        for payload in payloads:
            captured = json.loads(payload)
            if captured.get("kind") != "dante_note_widget":
                continue
            assert "Limbus Company Bus" in search(db, "Limbus Company Bus", 10)["passages"][0]["text_zh"]
            malformed_widget = copy.deepcopy(captured)
            source_slot = malformed_widget["page"]["revisions"][0]["slots"]["main"]
            source_slot["content"] = source_slot["content"].replace("const contentData", "const contentData = externalCall(); const ignored", 1)
            try:
                import_capture(db, archive, json.dumps(malformed_widget, ensure_ascii=False))
            except CaptureError:
                pass
            else:
                raise AssertionError("Executable widget expression was accepted")
            assert status(db) == baseline
        unrelated = root / "unrelated.sqlite3"
        with sqlite3.connect(unrelated) as conn:
            conn.execute("CREATE TABLE unrelated(id INTEGER PRIMARY KEY)")
        try:
            initialize(unrelated)
        except ValueError:
            pass
        else:
            raise AssertionError("Unrelated database was accepted")
        with sqlite3.connect(db) as conn:
            conn.execute("INSERT INTO passages_fts(passages_fts,rank) VALUES('integrity-check',1)")
            assert conn.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
            assert not conn.execute("PRAGMA foreign_key_check").fetchall()
    with sqlite3.connect(database.resolve().as_uri() + "?mode=ro", uri=True) as conn:
        assert conn.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
        assert not conn.execute("PRAGMA foreign_key_check").fetchall()
        for digest, path, render_hash in conn.execute("SELECT sha256,relative_path,render_sha256 FROM captures"):
            raw = (captures / path).read_bytes()
            assert hashlib.sha256(raw).hexdigest() == digest
            assert hashlib.sha256(parse_capture(raw.decode()).html.encode()).hexdigest() == render_hash
        for page_id, digest in conn.execute("SELECT page_id,capture_sha256 FROM pages"):
            assert parse_capture((captures / f"{digest}.json").read_text()).page_id == page_id
    assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    return {"status":"passed", "real_capture_count":len(payloads),
        "checks":["real capture import", "idempotence", "1/2/3-character Chinese search",
            "attribution", "context windows", "bounded passages", "dynamic placeholder exclusion",
            "wrong host/revision/missing source rejection", "unrelated database protection",
            "SQLite/foreign-key/FTS consistency", "capture/render hashes", "delivered database unchanged"],
        "database_sha256":before, "library":status(database)}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db", required=True, type=Path)
    parser.add_argument("--captures", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    result = verify(args.db, args.captures)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
