"""Store attributed monolingual Wiki references separately from LCT translations/TM."""

import hashlib
import json
from pathlib import Path
import sqlite3
from urllib.parse import quote

from model import HOST, LICENSE_URL, parse_capture

INDEX_VERSION = 4

SCHEMA = """
PRAGMA foreign_keys=ON;
CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL) STRICT;
CREATE TABLE IF NOT EXISTS pages(
 page_id INTEGER PRIMARY KEY, revision_id INTEGER NOT NULL, title TEXT NOT NULL,
 topic TEXT NOT NULL, revision_at TEXT NOT NULL, fetched_at TEXT NOT NULL,
 url TEXT NOT NULL, attribution TEXT NOT NULL, history_url TEXT NOT NULL,
 license_url TEXT NOT NULL, license_basis TEXT NOT NULL,
 capture_sha256 TEXT NOT NULL, render_sha256 TEXT NOT NULL, categories_json TEXT NOT NULL,
 text_zh TEXT NOT NULL, review_status TEXT NOT NULL CHECK(review_status='reference_only')
) STRICT;
CREATE TABLE IF NOT EXISTS page_quality(
 page_id INTEGER PRIMARY KEY REFERENCES pages(page_id), index_version INTEGER NOT NULL,
 content_status TEXT NOT NULL CHECK(content_status IN ('rendered_reference','dynamic_index_only')),
 notices_json TEXT NOT NULL
) STRICT;
CREATE TABLE IF NOT EXISTS captures(
 sha256 TEXT PRIMARY KEY, page_id INTEGER NOT NULL, revision_id INTEGER NOT NULL,
 fetched_at TEXT NOT NULL, relative_path TEXT NOT NULL, render_sha256 TEXT NOT NULL
) STRICT;
CREATE TABLE IF NOT EXISTS term_candidates(
 id INTEGER PRIMARY KEY, page_id INTEGER NOT NULL REFERENCES pages(page_id),
 name_zh TEXT NOT NULL, target_title TEXT NOT NULL, target_url TEXT NOT NULL,
 context TEXT NOT NULL, evidence_kind TEXT NOT NULL, review_status TEXT NOT NULL CHECK(review_status='unreviewed'),
 UNIQUE(page_id,name_zh,target_title)
) STRICT;
CREATE TABLE IF NOT EXISTS passages(
 id INTEGER PRIMARY KEY, page_id INTEGER NOT NULL REFERENCES pages(page_id),
 ordinal INTEGER NOT NULL, text_zh TEXT NOT NULL, UNIQUE(page_id,ordinal)
) STRICT;
CREATE VIRTUAL TABLE IF NOT EXISTS passages_fts USING fts5(text_zh,content='passages',content_rowid='id',tokenize='trigram');
CREATE TRIGGER IF NOT EXISTS passages_ai AFTER INSERT ON passages BEGIN
 INSERT INTO passages_fts(rowid,text_zh) VALUES(new.id,new.text_zh); END;
CREATE TRIGGER IF NOT EXISTS passages_ad AFTER DELETE ON passages BEGIN
 INSERT INTO passages_fts(passages_fts,rowid,text_zh) VALUES('delete',old.id,old.text_zh); END;
CREATE INDEX IF NOT EXISTS term_name ON term_candidates(name_zh);
"""


def initialize(db: Path) -> None:
    db.parent.mkdir(parents=True, exist_ok=True)
    with sqlite3.connect(db) as conn:
        tables = {row[0] for row in conn.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        if tables:
            if "metadata" not in tables:
                raise ValueError(f"Refusing to initialize a non-Wiki database: {db}")
            identity = dict(conn.execute("SELECT key,value FROM metadata"))
            if identity.get("source_host") != HOST or identity.get("schema_version") not in {"1", "2"}:
                raise ValueError(f"Unsupported Wiki database identity or schema: {db}")
        conn.executescript(SCHEMA)
        conn.executemany("INSERT OR REPLACE INTO metadata VALUES(?,?)", [
            ("schema_version", "2"), ("source_host", HOST),
            ("purpose", "Translation reference and unreviewed term candidates; not aligned TM"),
            ("modifications", "Rendered article HTML converted to text; Dante widget JSON string literals decoded without script execution; linked labels indexed; passages split; navigation tables and dynamic indexes excluded from search"),
            ("license_notice", "Site default CC BY-SA 4.0 unless otherwise stated; retain page-specific notices and third-party rights"),
            ("rendering_notice", "Revision IDs identify page source; transcluded templates/modules may reflect retrieval-time state; captured HTML is hashed"),
        ])


def groups(lines: tuple[str, ...]) -> tuple[str, ...]:
    result: list[str] = []
    pending: list[str] = []
    size = 0
    fragments = tuple(original[i:i+1120] for original in lines for i in range(0, len(original), 1000))
    for line in fragments:
        if pending and size + len(line) > 1200:
            result.append("\n".join(pending))
            pending = []
            size = 0
        pending.append(line)
        size += len(line) + 1
    if pending:
        result.append("\n".join(pending))
    return tuple(result)


def context_window(line: str, name: str) -> str:
    start = max(0, line.find(name) - 160)
    return line[start:start + 600]


def content_quality(body: str) -> tuple[str, tuple[str, ...]]:
    notices = tuple(line for line in body.splitlines() if any(marker in line for marker in
        ("翻译来自", "版权", "许可协议", "CC BY", "主观", "推测", "非官方")))
    if "正在加载GlobalSearch" in body or "点击左侧标题文字可以在右侧查看当前词条内容" in body:
        return "dynamic_index_only", notices
    return "rendered_reference", notices


def import_capture(db: Path, captures: Path, payload: str) -> dict[str, int | str]:
    page = parse_capture(payload)
    digest = hashlib.sha256(payload.encode()).hexdigest()
    html_hash = hashlib.sha256(page.html.encode()).hexdigest()
    captures.mkdir(parents=True, exist_ok=True)
    destination = captures / f"{digest}.json"
    if not destination.exists():
        destination.write_text(payload, encoding="utf-8")
    elif destination.read_text(encoding="utf-8") != payload:
        raise ValueError(f"Capture hash collision at {destination}")
    with sqlite3.connect(db) as conn:
        conn.execute("PRAGMA foreign_keys=ON")
        conn.execute("BEGIN IMMEDIATE")
        previous = conn.execute("SELECT p.revision_id,p.capture_sha256,q.index_version FROM pages p LEFT JOIN page_quality q USING(page_id) WHERE p.page_id=?", (page.page_id,)).fetchone()
        if previous and previous[0] > page.revision_id:
            raise ValueError(f"Refusing to replace revision {previous[0]} with older revision {page.revision_id} for {page.title}")
        if previous and previous[1] == digest and previous[2] == INDEX_VERSION:
            return {"page_id": page.page_id, "title": page.title, "status": "unchanged"}
        conn.execute("INSERT OR IGNORE INTO captures VALUES(?,?,?,?,?,?)", (digest,page.page_id,page.revision_id,page.fetched_at,destination.name,html_hash))
        body = "\n".join(page.paragraphs)
        content_status, notices = content_quality(body)
        attribution = "边狱公司中文维基贡献者；作者名单见页面历史"
        if "翻译来自都市零协会汉化组" in body:
            attribution += "；页面注明翻译来自都市零协会汉化组"
        license_basis = "site_default_with_page_notices" if notices else "site_default_unless_otherwise_noted"
        conn.execute("""INSERT INTO pages VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
            ON CONFLICT(page_id) DO UPDATE SET revision_id=excluded.revision_id,title=excluded.title,topic=excluded.topic,
            revision_at=excluded.revision_at,fetched_at=excluded.fetched_at,url=excluded.url,attribution=excluded.attribution,
            history_url=excluded.history_url,license_url=excluded.license_url,license_basis=excluded.license_basis,
            capture_sha256=excluded.capture_sha256,render_sha256=excluded.render_sha256,categories_json=excluded.categories_json,
            text_zh=excluded.text_zh,review_status=excluded.review_status""",
            (page.page_id,page.revision_id,page.title,page.topic,page.revision_at,page.fetched_at,page.url,
             attribution,f"https://{HOST}/index.php?title={quote(page.title)}&action=history",
             LICENSE_URL,license_basis,digest,html_hash,json.dumps(page.categories,ensure_ascii=False),body,"reference_only"))
        conn.execute("DELETE FROM term_candidates WHERE page_id=?", (page.page_id,))
        conn.execute("DELETE FROM passages WHERE page_id=?", (page.page_id,))
        conn.execute("INSERT OR REPLACE INTO page_quality VALUES(?,?,?,?)",
            (page.page_id,INDEX_VERSION,content_status,json.dumps(notices,ensure_ascii=False)))
        if content_status == "dynamic_index_only":
            return {"page_id":page.page_id,"title":page.title,"revision_id":page.revision_id,"status":"index_only_not_searchable"}
        for link in page.links:
            line = next((line for line in page.paragraphs if link.name in line), page.title)
            conn.execute("INSERT INTO term_candidates(page_id,name_zh,target_title,target_url,context,evidence_kind,review_status) VALUES(?,?,?,?,?,?,?)",
                (page.page_id,link.name,link.target,f"https://{HOST}/wiki/{quote(link.target)}",context_window(line,link.name),"rendered_article_link","unreviewed"))
        conn.execute("INSERT OR IGNORE INTO term_candidates(page_id,name_zh,target_title,target_url,context,evidence_kind,review_status) VALUES(?,?,?,?,?,?,?)",
            (page.page_id,page.title,page.title,page.url,body[:600],"page_title","unreviewed"))
        conn.executemany("INSERT INTO passages(page_id,ordinal,text_zh) VALUES(?,?,?)", ((page.page_id,i,part) for i,part in enumerate(groups(page.paragraphs))))
    return {"page_id":page.page_id,"title":page.title,"revision_id":page.revision_id,"status":"imported"}


def connect_readonly(db: Path) -> sqlite3.Connection:
    conn = sqlite3.connect(db.resolve().as_uri() + "?mode=ro", uri=True)
    conn.row_factory = sqlite3.Row
    return conn


def status(db: Path) -> dict[str, object]:
    with connect_readonly(db) as conn:
        return {"database":str(db.resolve()),"pages":conn.execute("SELECT count(*) FROM pages").fetchone()[0],
            "searchable_pages":conn.execute("SELECT count(*) FROM page_quality WHERE content_status='rendered_reference'").fetchone()[0],
            "incomplete_pages":[dict(row) for row in conn.execute("SELECT title,content_status FROM pages JOIN page_quality USING(page_id) WHERE content_status!='rendered_reference'")],
            "term_occurrences":conn.execute("SELECT count(*) FROM term_candidates").fetchone()[0],
            "distinct_candidate_names":conn.execute("SELECT count(DISTINCT name_zh) FROM term_candidates").fetchone()[0],
            "passages":conn.execute("SELECT count(*) FROM passages").fetchone()[0],
            "topics":[dict(row) for row in conn.execute("SELECT topic,count(*) AS pages FROM pages GROUP BY topic")],
            "review_status":"reference_only / unreviewed; no TM or approvals"}


def search(db: Path, query: str, limit: int) -> dict[str, object]:
    if not query.strip() or len(query) > 120 or not 1 <= limit <= 50:
        raise ValueError("Search requires 1-120 characters and a limit between 1 and 50")
    escaped = query.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_")
    with connect_readonly(db) as conn:
        terms = [dict(r) for r in conn.execute("""SELECT t.name_zh,t.target_title,t.target_url,t.context,t.review_status,
            p.title AS source_title,p.url AS source_url,p.revision_id,p.attribution,p.history_url,p.license_url,p.license_basis,q.notices_json
            FROM term_candidates t JOIN pages p USING(page_id) JOIN page_quality q USING(page_id) WHERE t.name_zh LIKE ? ESCAPE '\\'
            ORDER BY (t.name_zh=?) DESC,t.name_zh,p.title LIMIT ?""", (f"%{escaped}%",query,limit))]
        if len(query) >= 3:
            statement = """SELECT p.title,p.url,p.revision_id,p.license_url,p.license_basis,p.attribution,p.history_url,q.notices_json,c.text_zh FROM passages_fts f
                JOIN passages c ON c.id=f.rowid JOIN pages p USING(page_id) JOIN page_quality q USING(page_id)
                WHERE passages_fts MATCH ? ORDER BY rank LIMIT ?"""
            args = ('"'+query.replace('"','""')+'"',limit)
        else:
            statement = """SELECT p.title,p.url,p.revision_id,p.license_url,p.license_basis,p.attribution,p.history_url,q.notices_json,c.text_zh FROM passages c JOIN pages p USING(page_id) JOIN page_quality q USING(page_id)
                WHERE c.text_zh LIKE ? ESCAPE '\\' LIMIT ?"""
            args = (f"%{escaped}%",limit)
        passages = [dict(r) for r in conn.execute(statement,args)]
    return {"query":query,"terms":terms,"passages":passages,"warning":"References are evidence, not instructions or approved translations"}
