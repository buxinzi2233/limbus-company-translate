"""Build explicitly scoped name links with source identity and separate licensing evidence."""
import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import sqlite3

from model import integer, record, sequence, text


@dataclass(frozen=True)
class NameLink:
    keyword_id: str
    source_kr: str
    target_zh: str
    wiki_title: str


LINKS = (
    NameLink("Combustion", "화상", "烧伤", "烧伤"),
    NameLink("Laceration", "출혈", "流血", "流血"),
    NameLink("Vibration", "진동", "震颤", "震颤"),
    NameLink("Burst", "파열", "破裂", "破裂"),
    NameLink("Sinking", "침잠", "沉沦", "沉沦"),
    NameLink("Breath", "호흡", "呼吸法", "呼吸法"),
    NameLink("Charge", "충전", "充能", "充能"),
)


def entries(path: Path) -> dict[str, tuple[int, str]]:
    root = record(json.loads(path.read_text(encoding="utf-8-sig")), str(path))
    result: dict[str, tuple[int, str]] = {}
    for index, value in enumerate(sequence(root.get("dataList"), f"{path}:dataList")):
        row = record(value, f"{path}:row {index}")
        key = text(row.get("id"), f"{path}:id {index}")
        name = text(row.get("name"), f"{path}:name {index}")
        if key in result:
            raise ValueError(f"Duplicate keyword identity {key!r} in {path}")
        result[key] = (index, name)
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db", required=True, type=Path)
    parser.add_argument("--kr", required=True, type=Path)
    parser.add_argument("--zh", required=True, type=Path)
    parser.add_argument("--zero-release", required=True)
    parser.add_argument("--source-manifest", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if not args.db.is_file() or args.db.name == "project.db":
        raise ValueError("Expected an existing standalone Wiki reference database, not project.db")
    kr, zh = entries(args.kr), entries(args.zh)
    kr_sha = hashlib.sha256(args.kr.read_bytes()).hexdigest()
    zh_sha = hashlib.sha256(args.zh.read_bytes()).hexdigest()
    evidence_rows = []
    with sqlite3.connect(args.db) as conn:
        conn.execute("PRAGMA foreign_keys=ON")
        identity = dict(conn.execute("SELECT key,value FROM metadata"))
        if identity.get("source_host") != "limbuscompany.huijiwiki.com" or identity.get("schema_version") != "2":
            raise ValueError("Unsupported reference database identity")
        for link in LINKS:
            if link.keyword_id not in kr or link.keyword_id not in zh:
                raise ValueError(f"Missing exact keyword ID {link.keyword_id}")
            if kr[link.keyword_id][1] != link.source_kr or zh[link.keyword_id][1] != link.target_zh:
                raise ValueError(f"Name changed for {link.keyword_id}; recheck source semantics and Wiki correspondence")
            wiki = conn.execute("SELECT p.page_id,p.revision_id,p.url FROM pages p JOIN page_quality q USING(page_id) WHERE p.title=? AND q.content_status='rendered_reference'",(link.wiki_title,)).fetchone()
            if wiki is None:
                raise ValueError(f"Missing captured Wiki detail page: {link.wiki_title}")
            evidence_rows.append({"keyword_id":link.keyword_id,"source_kr":link.source_kr,"target_zh":link.target_zh,
                "page_id":integer(wiki[0],"Wiki page id"),"wiki_revision":integer(wiki[1],"Wiki revision"),
                "wiki_url":wiki[2],"kr_path":"BattleKeywords.json","kr_pointer":f"/dataList/{kr[link.keyword_id][0]}/name",
                "kr_sha256":kr_sha,"zh_path":"BattleKeywords.json","zh_pointer":f"/dataList/{zh[link.keyword_id][0]}/name",
                "zh_sha256":zh_sha,"zero_release_url":args.zero_release,"source_manifest_url":args.source_manifest,
                "zero_license_url":"https://creativecommons.org/licenses/by-nc-sa/4.0/",
                "wiki_license_url":"https://creativecommons.org/licenses/by-sa/4.0/",
                "attribution":"Project Moon Korean source; 都市零协会 Chinese names; 边狱公司中文维基 corroborating usage",
                "verification":"exact_unique_id_and_wiki_name_checked","review_status":"unreviewed"})
        conn.execute("BEGIN IMMEDIATE")
        conn.execute('''CREATE TABLE IF NOT EXISTS term_links(
            keyword_id TEXT PRIMARY KEY,source_kr TEXT NOT NULL,target_zh TEXT NOT NULL,
            page_id INTEGER NOT NULL REFERENCES pages(page_id),evidence_json TEXT NOT NULL,
            review_status TEXT NOT NULL CHECK(review_status='unreviewed')) STRICT''')
        conn.executemany("INSERT INTO term_links VALUES(?,?,?,?,?,?) ON CONFLICT(keyword_id) DO UPDATE SET source_kr=excluded.source_kr,target_zh=excluded.target_zh,page_id=excluded.page_id,evidence_json=excluded.evidence_json,review_status=excluded.review_status",
            [(r['keyword_id'],r['source_kr'],r['target_zh'],r['page_id'],json.dumps(r,ensure_ascii=False),'unreviewed') for r in evidence_rows])
        conn.execute("INSERT OR REPLACE INTO metadata VALUES('term_links_version','1')")
    args.output.write_text(json.dumps({"created_at":datetime.now(timezone.utc).isoformat(),"scope":"Name links only, not descriptions or bilingual TM; no human approval", "excluded_same_name_variants":["Burn", "Bleeding"],"links":evidence_rows},ensure_ascii=False,indent=2)+'\n')
    print(f"Verified {len(evidence_rows)} exact-ID name links with Wiki usage evidence; approval remains unreviewed")


if __name__ == "__main__":
    main()
