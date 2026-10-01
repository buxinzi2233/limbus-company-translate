"""Validate public MediaWiki captures without accepting instructions from page text."""

from dataclasses import dataclass
from datetime import datetime
from html.parser import HTMLParser
import json
import re
from urllib.parse import parse_qs, unquote, urlsplit

HOST = "limbuscompany.huijiwiki.com"
LICENSE_URL = "https://creativecommons.org/licenses/by-sa/4.0/"


class CaptureError(ValueError):
    """A capture is incomplete, inconsistent, or outside the selected wiki."""


def record(value: object, location: str) -> dict[str, object]:
    if not isinstance(value, dict) or not all(isinstance(k, str) for k in value):
        raise CaptureError(f"{location}: expected an object with string keys")
    return value


def text(value: object, location: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise CaptureError(f"{location}: expected a nonempty string")
    return value


def integer(value: object, location: str) -> int:
    if type(value) is not int or value < 1:
        raise CaptureError(f"{location}: expected a positive integer")
    return value


def sequence(value: object, location: str) -> list[object]:
    if not isinstance(value, list):
        raise CaptureError(f"{location}: expected a list")
    return value


def wiki_url(value: object, location: str) -> str:
    url = text(value, location)
    parsed = urlsplit(url)
    if parsed.scheme != "https" or parsed.netloc != HOST:
        raise CaptureError(f"{location}: expected HTTPS on {HOST}, received {url!r}")
    return url


@dataclass(frozen=True)
class Link:
    name: str
    target: str


@dataclass(frozen=True)
class Page:
    page_id: int
    revision_id: int
    title: str
    topic: str
    revision_at: str
    fetched_at: str
    url: str
    wikitext: str
    html: str
    categories: tuple[str, ...]
    links: tuple[Link, ...]
    paragraphs: tuple[str, ...]


class ArticleParser(HTMLParser):
    """Parse rendered article HTML through the standard parser interface."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.parts: list[str] = []
        self.links: list[Link] = []
        self.anchor_target: str | None = None
        self.anchor_parts: list[str] = []
        self.hidden = 0
        self.navigation_table_depth = 0

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        attributes = dict(attrs)
        if self.navigation_table_depth:
            if tag == "table":
                self.navigation_table_depth += 1
            return
        if tag == "table" and "navbox" in (attributes.get("class") or "").split():
            self.navigation_table_depth = 1
            return
        if tag in {"script", "style"}:
            self.hidden += 1
        if self.hidden:
            return
        if tag in {"p", "div", "li", "tr", "br", "h1", "h2", "h3", "h4", "h5", "h6"}:
            self.parts.append("\n")
        if tag in {"td", "th"}:
            self.parts.append(" | ")
        if tag == "a":
            href = attributes.get("href") or ""
            parsed = urlsplit(href)
            if (not parsed.netloc or parsed.netloc == HOST) and parsed.path.startswith("/wiki/"):
                title = unquote(parsed.path[6:]).replace("_", " ")
                if not any(title.startswith(prefix) for prefix in ("文件:", "File:", "分类:", "Category:", "模板:", "Template:", "特殊:", "Special:", "用户:", "User:", "讨论:", "Talk:")) and "action=edit" not in href:
                    self.anchor_target = title
                    self.anchor_parts = []

    def handle_endtag(self, tag: str) -> None:
        if self.navigation_table_depth:
            if tag == "table":
                self.navigation_table_depth -= 1
            return
        if tag in {"script", "style"} and self.hidden:
            self.hidden -= 1
            return
        if self.hidden:
            return
        if tag == "a" and self.anchor_target is not None:
            name = " ".join("".join(self.anchor_parts).split())
            if name and len(name) <= 160:
                self.links.append(Link(name, self.anchor_target))
            self.anchor_target = None
            self.anchor_parts = []
        if tag in {"p", "div", "li", "tr", "h1", "h2", "h3", "h4", "h5", "h6"}:
            self.parts.append("\n")

    def handle_data(self, data: str) -> None:
        if not self.hidden and not self.navigation_table_depth:
            self.parts.append(data)
            if self.anchor_target is not None:
                self.anchor_parts.append(data)


def parse_capture(payload: str) -> Page:
    root = record(json.loads(payload), "capture")
    fetched = text(root.get("fetched_at"), "fetched_at")
    datetime.fromisoformat(fetched.replace("Z", "+00:00"))
    topic = text(root.get("topic"), "topic")
    query_url = wiki_url(root.get("query_url"), "query_url")
    if urlsplit(query_url).path != "/api.php" or parse_qs(urlsplit(query_url).query).get("action") != ["query"]:
        raise CaptureError("query_url must identify the public query API")
    metadata = record(root.get("page"), "page")
    widget = root.get("kind") == "dante_note_widget"
    expected_namespace = 274 if widget else 0
    if widget and metadata.get("title") != "Html:Dantenote":
        raise CaptureError("The widget importer only accepts Html:Dantenote")
    if metadata.get("ns") != expected_namespace or metadata.get("missing") or metadata.get("invalid"):
        raise CaptureError("Only existing main-namespace pages may enter this reference library")
    page_id = integer(metadata.get("pageid"), "page.pageid")
    title = text(metadata.get("title"), "page.title")
    url = wiki_url(metadata.get("fullurl"), "page.fullurl")
    revisions = sequence(metadata.get("revisions"), "page.revisions")
    if len(revisions) != 1:
        raise CaptureError("A capture must contain exactly one current revision")
    revision = record(revisions[0], "revision")
    revision_id = integer(revision.get("revid"), "revision.revid")
    revision_at = text(revision.get("timestamp"), "revision.timestamp")
    datetime.fromisoformat(revision_at.replace("Z", "+00:00"))
    slots = record(revision.get("slots"), "revision.slots")
    main = record(slots.get("main"), "revision.slots.main")
    source = text(main.get("content"), "revision.slots.main.content")
    if widget:
        owner = record(root.get("owner_page"), "owner_page")
        if owner.get("title") != "但丁笔记" or owner.get("ns") != 0:
            raise CaptureError("Dante widget requires its main-namespace owner page")
        owner_revisions = sequence(owner.get("revisions"), "owner revisions")
        if len(owner_revisions) != 1:
            raise CaptureError("Expected one owner revision")
        owner_revision = record(owner_revisions[0], "owner revision")
        integer(owner_revision.get("revid"), "owner revision ID")
        owner_main = record(record(owner_revision.get("slots"), "owner slots").get("main"), "owner main")
        if "{{#html:Dantenote}}" not in text(owner_main.get("content"), "owner source"):
            raise CaptureError("Owner source does not include Html:Dantenote")
        content = literal_dante_content(source)
        html = "<p>但丁笔记：翻译来自都市零协会汉化组；保留第三方权利与页面特别声明。</p>" + content.replace("\n", "<br>")
        parser = ArticleParser()
        parser.feed(html)
        parser.close()
        paragraphs = tuple(line.strip() for line in "".join(parser.parts).splitlines() if line.strip())
        if len(paragraphs) < 2:
            raise CaptureError("Dante widget contains no usable note content")
        return Page(page_id, revision_id, title, topic, revision_at, fetched, url, source, html, (), tuple(dict.fromkeys(parser.links)), paragraphs)
    render_url = wiki_url(root.get("render_url"), "render_url")
    params = parse_qs(urlsplit(render_url).query)
    if urlsplit(render_url).path != "/api.php" or params.get("action") != ["parse"] or params.get("oldid") != [str(revision_id)]:
        raise CaptureError("Rendered content must be requested by the captured revision ID")
    response = record(root.get("render"), "render response")
    if "error" in response:
        raise CaptureError(f"MediaWiki parse error: {response['error']}")
    rendered = record(response.get("parse"), "parse")
    if rendered.get("pageid") != page_id or rendered.get("revid") != revision_id:
        raise CaptureError("Rendered page ID/revision does not match the source capture")
    html = text(rendered.get("text"), "parse.text")
    parser = ArticleParser()
    parser.feed(html)
    parser.close()
    paragraphs = tuple(line.strip(" |\t\r") for line in "".join(parser.parts).splitlines() if line.strip(" |\t\r"))
    if not paragraphs:
        raise CaptureError(f"No rendered text extracted from {title!r}")
    categories = tuple(text(record(item, "category").get("title"), "category.title") for item in sequence(metadata.get("categories", []), "categories"))
    return Page(page_id, revision_id, title, topic, revision_at, fetched, url, source, html, categories, tuple(dict.fromkeys(parser.links)), paragraphs)


def literal_dante_content(source: str) -> str:
    """Decode only JSON string literals joined by plus; never execute JavaScript."""
    marker = "const contentData"
    if source.count(marker) != 1:
        raise CaptureError("Expected exactly one Dante contentData declaration")
    expression = source.split(marker, 1)[1].lstrip()
    if not expression.startswith("="):
        raise CaptureError("Dante contentData must be a literal assignment")
    remaining = expression[1:]
    parts: list[str] = []
    decoder = json.JSONDecoder()
    while True:
        remaining = re.sub(r"^(?:\s+|//[^\n]*(?:\n|$))*", "", remaining)
        if not remaining.startswith('"'):
            raise CaptureError("Dante contentData contains a nonliteral expression")
        try:
            value, end = decoder.raw_decode(remaining)
        except json.JSONDecodeError as error:
            raise CaptureError(f"Invalid Dante string literal: {error}") from error
        if not isinstance(value, str):
            raise CaptureError("Dante contentData item is not a string")
        parts.append(value)
        remaining = re.sub(r"^(?:\s+|//[^\n]*(?:\n|$))*", "", remaining[end:])
        if remaining.startswith(";"):
            return "".join(parts)
        if not remaining.startswith("+"):
            raise CaptureError("Dante string must end in a semicolon or literal concatenation")
        remaining = remaining[1:]
