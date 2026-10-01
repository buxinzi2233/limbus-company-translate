#!/usr/bin/env python3
"""Exercise a packaged executable over real HTTP and CLI using isolated files only."""

import hashlib
import json
import sqlite3
import re
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path


def invoke(binary: Path, data: Path, arguments: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(binary), "--data-dir", str(data), "--json", *arguments],
        cwd=data.parent, check=True, capture_output=True, text=True,
    )


def request(base: str, path: str, body: dict[str, str | int] | None) -> tuple[int, bytes]:
    headers = {"Content-Type": "application/json"}
    payload = None if body is None else json.dumps(body).encode()
    call = urllib.request.Request(base + path, data=payload, headers=headers)
    try:
        with urllib.request.urlopen(call, timeout=5) as response:
            return response.status, response.read()
    except urllib.error.HTTPError as error:
        return error.code, error.read()


def write_source(path: Path, records: list[dict[str, str | int]]) -> None:
    path.write_text(json.dumps({"items": records}, ensure_ascii=False), encoding="utf-8")


def import_fixture_scope(binary: Path, data: Path, source: Path) -> tuple[str, int]:
    """Validate fixture membership through the real CLI after each source snapshot."""
    with sqlite3.connect(f"file:{data / 'project.db'}?mode=ro", uri=True) as connection:
        records = connection.execute(
            "SELECT s.id,f.relative_path,s.key_path,s.source_identity,s.source_kr "
            "FROM segments s JOIN files f ON f.id=s.file_id "
            "WHERE s.project_id='acceptance' AND s.active=1 ORDER BY s.id"
        ).fetchall()
    for record in records:
        if not isinstance(record[0], int) or not all(isinstance(value, str) for value in record[1:]):
            raise TypeError("Fixture scope records require an integer ID and string source identity fields")
    manifest = data.parent / "scope.json"
    manifest.write_text(json.dumps({
        "project_id": "acceptance",
        "files": [{"path": "a.json", "sha256": hashlib.sha256(source.read_bytes()).hexdigest(), "selected_segments": len(records)}],
        "segments": [{"segment_id": identifier, "path": path, "pointer": pointer,
                      "source_identity": identity, "source_kr": korean}
                     for identifier, path, pointer, identity, korean in records],
    }), encoding="utf-8")
    scope = json.loads(invoke(binary, data, ["scope-import", "--project-id", "acceptance",
        "--name", "Isolated release acceptance", "--input", str(manifest)]).stdout)
    if not isinstance(scope["manifest_sha256"], str) or not isinstance(scope["source_import_id"], int):
        raise TypeError("Scope receipt requires manifest_sha256 and source_import_id")
    return scope["manifest_sha256"], scope["source_import_id"]


def accept(binary: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="lct-release-http-") as temporary:
        root = Path(temporary)
        data, kr, zh = root / "data", root / "kr", root / "zh"
        kr.mkdir()
        zh.mkdir()
        source = kr / "a.json"
        first = {"id": 1, "text": "체력 {0} 20%"}
        second = {"id": 2, "text": "안녕하세요"}
        write_source(source, [first, second])
        extraction = ["extract", "--project-id", "acceptance", "--kr", str(kr), "--zh", str(zh)]
        invoke(binary, data, extraction)
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        base = f"http://127.0.0.1:{port}"
        with (root / "server.log").open("w", encoding="utf-8") as log:
            server = subprocess.Popen(
                [str(binary), "--data-dir", str(data), "ui", "--project-id", "acceptance", "--port", str(port), "--no-open"],
                cwd=root, stdout=log, stderr=log,
            )
            try:
                for attempt in range(100):
                    if server.poll() is not None:
                        raise RuntimeError(f"Server exited: {(root / 'server.log').read_text()}")
                    try:
                        status, _ = request(base, "/api/status", None)
                        if status == 200:
                            break
                    except urllib.error.URLError:
                        time.sleep(0.1)
                else:
                    raise TimeoutError("Release server did not become ready within 10 seconds")
                status, html = request(base, "/", None)
                assert status == 200
                assets = re.findall(r'(?:src|href)="(/[^" ]+)"', html.decode())
                assert any(asset.endswith(".js") for asset in assets)
                for asset in assets:
                    code, content = request(base, asset, None)
                    assert code == 200 and content, asset
                _, payload = request(base, "/api/segments", None)
                rows = json.loads(payload)["items"]
                assert len(rows) == 2
                row = next(item for item in rows if item["key_path"] == "/items/0/text")
                digest, import_id = import_fixture_scope(binary, data, source)
                grant_request = root / "grant.json"
                permit_path = root / "permit.json"
                grant_request.write_text(json.dumps({"project_id":"acceptance", "worker":"release-acceptance",
                    "manifest_sha256":digest, "source_import_id":import_id, "segment_ids":[row["id"]]}), encoding="utf-8")
                grant = json.loads(invoke(binary, data, ["draft-grant", "--input", str(grant_request), "--output", str(permit_path)]).stdout)
                draft_path = root / "draft.json"
                draft_path.write_text(json.dumps({"project_id": "acceptance", "id": row["id"],
                    "expected_updated_at": row["updated_at"], "target_zh": "生命{0}20%",
                    "producer": "acceptance-agent", "stage": 4}), encoding="utf-8")
                draft = json.loads(invoke(binary, data, ["draft", "--draft-permit", str(permit_path), "--input", str(draft_path)]).stdout)["item"]
                assert draft["stage"] == "AiDrafted"
                row = draft
                invoke(binary, data, ["draft-revoke", "--project-id", "acceptance", "--grant-id", grant["id"]])
                _, queue_payload = request(base, "/api/review-queue", None)
                assert any(item["id"] == row["id"] for item in json.loads(queue_payload)["items"])
                empty_injection = json.loads(invoke(binary, data, ["inject", "--project-id", "acceptance", "--workspace", str(zh), "--kr", str(kr)]).stdout)
                assert empty_injection["modified_files"] == 0
                _, audit_payload = request(base, "/api/audit", None)
                assert json.loads(audit_payload)["events"][0]["kind"] == "ai_draft"
                change = {"id": row["id"], "project_id": "acceptance", "stage": 4,
                          "expected_updated_at": row["updated_at"], "target_zh": "生命20%"}
                assert request(base, "/api/segments/update", change)[0] == 422
                change["target_zh"] = "生命{0}20%"
                assert request(base, "/api/segments/update", change)[0] == 200
                assert request(base, "/api/segments/update", change)[0] == 409
                write_source(source, [second, first])
                invoke(binary, data, extraction)
                import_fixture_scope(binary, data, source)
                _, payload = request(base, "/api/segments", None)
                moved = next(item for item in json.loads(payload)["items"] if item["id"] == row["id"])
                assert moved["key_path"] == "/items/1/text" and moved["target_zh"] == "生命{0}20%"
                write_source(source, [second])
                invoke(binary, data, extraction)
                import_fixture_scope(binary, data, source)
                assert request(base, "/api/segments/update", change)[0] == 404
                _, payload = request(base, "/api/status", None)
                assert json.loads(payload)["stats"]["total_segments"] == 1
                _, payload = request(base, "/api/segments", None)
                remaining = json.loads(payload)["items"][0]
                save = {"id": remaining["id"], "project_id": "acceptance", "stage": 4,
                        "expected_updated_at": remaining["updated_at"], "target_zh": "你好"}
                assert request(base, "/api/segments/update", save)[0] == 200
                injected = json.loads(invoke(binary, data, ["inject", "--project-id", "acceptance", "--workspace", str(zh), "--kr", str(kr)]).stdout)
                assert injected["injected_segments"] == 1
                assert json.loads((zh / "a.json").read_text())["items"][0]["text"] == "你好"
                invoke(binary, data, ["restore", "--workspace", str(zh), "--backup", injected["backup_directory"]])
                assert not (zh / "a.json").exists()
                _, audit_payload = request(base, "/api/audit", None)
                audit = json.loads(audit_payload)
                assert audit["events"][0]["kind"] == "review"
                assert audit["imports"][0]["retired"] == 1
                assert audit["project"]["live_client_kr_dir"] == str(kr)
                assert [operation["kind"] for operation in audit["operations"]] == ["restore", "inject"]
                assert all(operation["project_id"] == "acceptance" for operation in audit["operations"])

                repo = Path(__file__).resolve().parents[1]
                subprocess.run(["pnpm", "--dir", str(repo / "ui"), "test:smoke", base], check=True)
                print("发布包真实 HTTP/CLI 验收通过：内嵌资源、AI 草稿、待审队列、QA 拒写、人工审校、版本冲突、数组重排、删除退出、项目绑定、注入恢复及审计")
            finally:
                server.terminate()
                try:
                    server.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait()


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("Usage: python3 scripts/accept-release.py /absolute/lct")
    accept(Path(sys.argv[1]).resolve(strict=True))
