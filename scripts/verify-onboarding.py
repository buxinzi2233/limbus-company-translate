#!/usr/bin/env python3
"""Verify generated new-user configuration against a real bundle outside its directory."""

import argparse
import hashlib
import json
from pathlib import Path
import socket
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

from mcp_transport import rpc, stop


def verify(binary: Path, bundle: Path) -> dict[str, object]:
    catalog = json.loads((bundle / "research/catalog.json").read_text(encoding="utf-8"))
    for document in catalog["documents"]:
        actual = hashlib.sha256((bundle / "research" / document["id"]).read_bytes()).hexdigest()
        assert actual == document["sha256"], document["id"]
    assert len(catalog["documents"]) == 65
    historical_tools = json.loads((bundle / "tools/manifest.json").read_text(encoding="utf-8"))
    assert len(historical_tools["files"]) == 15
    for entry in historical_tools["files"]:
        assert hashlib.sha256((bundle / "tools" / entry["path"]).read_bytes()).hexdigest() == entry["sha256"]
    with tempfile.TemporaryDirectory(prefix="lct-new-user-") as temporary:
        root = Path(temporary)
        workspace = root / "workspace with spaces"
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        subprocess.run(["python3", str(bundle / "scripts/setup-workspace.py"), "--binary", str(binary),
                        "--workspace", str(workspace), "--project-id", "fresh", "--name", "Fresh user",
                        "--port", str(port)], cwd=root, check=True, capture_output=True, text=True)
        config = json.loads((workspace / "mcp.json").read_text(encoding="utf-8"))["mcpServers"]["lct"]
        assert isinstance(config["command"], str) and isinstance(config["args"], list)
        assert all(isinstance(arg, str) for arg in config["args"])
        assert "--draft-permit" not in config["args"]
        reference = workspace / "references/references.sqlite3"
        reference_hash = hashlib.sha256(reference.read_bytes()).hexdigest()
        database = workspace / "data/project.db"
        with sqlite3.connect(database.as_uri() + "?mode=ro", uri=True) as conn:
            before = list(conn.iterdump())
        with (root / "mcp.log").open("w") as log:
            process = subprocess.Popen([config["command"], *config["args"]], cwd=root,
                                       stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=log,
                                       text=True, bufsize=1)
            try:
                rpc(process, 1, "initialize", {"protocolVersion":"2025-11-25", "capabilities":{},
                                              "clientInfo":{"name":"onboarding-check","version":"1"}})
                assert process.stdin
                process.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
                process.stdin.flush()
                tools = rpc(process, 2, "tools/list", {})["tools"]
                assert len(tools) == 13
                calls = [
                    ("project_status", {"project_id":"fresh"}),
                    ("list_research", {"project_id":"fresh","offset":0,"limit":50}),
                    ("search_research", {"project_id":"fresh","query":"文风","limit":5}),
                    ("read_research", {"project_id":"fresh","document_id":catalog["documents"][0]["id"]}),
                    ("search_corpus", {"project_id":"fresh","query":"经理","limit":5}),
                    ("search_references", {"project_id":"fresh","query":"呼吸法","limit":5}),
                ]
                replies = {}
                for request_id, (name, arguments) in enumerate(calls, start=3):
                    reply = rpc(process, request_id, "tools/call", {"name":name, "arguments":arguments})
                    assert not reply.get("isError"), reply
                    replies[name] = reply["structuredContent"]
                assert replies["project_status"]["total"] == 0
                assert replies["list_research"]["total"] == 65
                assert replies["search_research"]["total"] > 0
                assert replies["read_research"]["metadata"]["status"] == "historical_unverified"
                assert replies["search_corpus"]["items"] and replies["search_references"]["terms"]
                denied = rpc(process, 20, "tools/call", {"name":"submit_draft", "arguments":{
                    "project_id":"fresh", "id":1, "expected_updated_at":"2026-01-01T00:00:00Z",
                    "target_zh":"不得写入", "producer":"onboarding-check"}})
                assert denied["isError"] and denied["structuredContent"]["code"] == "write_forbidden"
            finally:
                stop(process)
        with (root / "ui.log").open("w") as log:
            process = subprocess.Popen([str(workspace / "start-ui.sh")], cwd=root, stdout=log, stderr=log)
            try:
                for attempt in range(100):
                    if process.poll() is not None:
                        raise RuntimeError((root / "ui.log").read_text())
                    try:
                        with urllib.request.urlopen(f"http://127.0.0.1:{port}/api/status", timeout=2) as response:
                            status = json.load(response)
                        break
                    except urllib.error.URLError:
                        time.sleep(0.1)
                else:
                    raise TimeoutError("Generated UI launcher did not start within 10 seconds")
                assert status["project"]["id"] == "fresh"
                with urllib.request.urlopen(f"http://127.0.0.1:{port}/", timeout=2) as response:
                    assert 'id="root"' in response.read().decode()
            finally:
                stop(process)
        with sqlite3.connect(database.as_uri() + "?mode=ro", uri=True) as conn:
            assert list(conn.iterdump()) == before
        assert hashlib.sha256(reference.read_bytes()).hexdigest() == reference_hash
    return {"status":"passed", "research_reports":65, "historical_tool_files":15,
            "mcp_tools":13, "checks":["fresh workspace outside bundle", "generated MCP configuration",
            "research listing/search/full read", "Chinese corpus and attributed Wiki search",
            "no automatic draft permission", "generated UI launcher and embedded frontend",
            "workspace and reference data unchanged by reads"]}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = verify(args.binary.resolve(strict=True), Path(__file__).resolve().parents[1])
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
