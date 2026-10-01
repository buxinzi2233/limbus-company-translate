#!/usr/bin/env python3
"""Validate reference integration using a real captured library and isolated LCT processes."""

import argparse
import hashlib
import json
from pathlib import Path
import selectors
import socket
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request


def http(base: str, path: str) -> tuple[int, object]:
    try:
        with urllib.request.urlopen(base + path, timeout=10) as response:
            return response.status, json.loads(response.read())
    except urllib.error.HTTPError as error:
        return error.code, json.loads(error.read())


def rpc(process: subprocess.Popen, request_id: int, method: str, parameters: dict[str, object]) -> dict[str, object]:
    assert process.stdin and process.stdout
    process.stdin.write(json.dumps({"jsonrpc":"2.0", "id":request_id, "method":method, "params":parameters}) + "\n")
    process.stdin.flush()
    with selectors.DefaultSelector() as selector:
        selector.register(process.stdout, selectors.EVENT_READ)
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if not selector.select(max(0, deadline - time.monotonic())):
                raise TimeoutError(f"MCP {method} request {request_id} exceeded 15 seconds")
            line = process.stdout.readline()
            if not line:
                raise RuntimeError(f"MCP stopped during {method}; exit={process.poll()}")
            result = json.loads(line)
            if result.get("id") == request_id:
                if "error" in result:
                    raise RuntimeError(f"MCP {method}: {result['error']}")
                return result["result"]
    raise TimeoutError(f"MCP {method} did not respond")


def stop(process: subprocess.Popen) -> None:
    process.terminate()
    process.wait(timeout=10)


def verify(binary: Path, reference: Path) -> dict[str, object]:
    digest = hashlib.sha256(reference.read_bytes()).hexdigest()
    with tempfile.TemporaryDirectory(prefix="lct-reference-integration-") as folder:
        root = Path(folder)
        base_command = [str(binary), "--data-dir", str(root)]
        subprocess.run(base_command + ["init", "reference-check"], check=True, capture_output=True, text=True)
        with sqlite3.connect((root / "project.db").as_uri() + "?mode=ro", uri=True) as conn:
            work_before = list(conn.iterdump())
        command = base_command + ["--reference-db", str(reference)]
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        base = f"http://127.0.0.1:{port}"
        with (root / "http.log").open("w") as log:
            process = subprocess.Popen(command + ["ui", "--project-id", "reference-check", "--port", str(port), "--no-open"], stdout=log, stderr=log)
            try:
                for attempt in range(100):
                    if process.poll() is not None:
                        raise RuntimeError((root / "http.log").read_text())
                    try:
                        code, coverage = http(base, "/api/references/status")
                        break
                    except urllib.error.URLError:
                        time.sleep(0.1)
                else:
                    raise TimeoutError("HTTP reference endpoint did not start within 10 seconds")
                assert code == 200 and coverage["configured"] and coverage["searchable_pages"] > 0
                for query in ("李", "李箱", "呼吸法"):
                    code, result = http(base, "/api/references/search?" + urllib.parse.urlencode({"q":query,"limit":5}))
                    assert code == 200 and result["terms"] and result["passages"], (code, result)
                    for item in result["terms"] + result["passages"]:
                        assert item["source"]["attribution"] and item["source"]["history_url"] and item["source"]["revision_id"]
                for query in ("", "x" * 121):
                    assert http(base, "/api/references/search?" + urllib.parse.urlencode({"q":query,"limit":5}))[0] == 400
                assert http(base, "/api/references/search?q=test&limit=51")[0] == 400
                assert http(base, "/api/references/search?q=test")[0] == 400
                code, result = http(base, "/api/references/search?" + urllib.parse.urlencode({"q":"正在加载GlobalSearch","limit":5}))
                assert code == 200 and not result["passages"]
            finally:
                stop(process)
        with (root / "mcp.log").open("w") as log:
            process = subprocess.Popen(command + ["mcp", "--project-id", "reference-check"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=log, text=True, bufsize=1)
            try:
                rpc(process, 1, "initialize", {"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"reference-integration","version":"1"}})
                process.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
                process.stdin.flush()
                tools = rpc(process, 2, "tools/list", {})["tools"]
                for name in ("search_references", "reference_status"):
                    tool = next(item for item in tools if item["name"] == name)
                    assert tool["annotations"]["readOnlyHint"] is True
                result = rpc(process, 3, "tools/call", {"name":"search_references","arguments":{"project_id":"reference-check","query":"呼吸法","limit":5}})
                assert not result.get("isError"), result
                assert result["structuredContent"]["terms"][0]["name"] == "呼吸法"
                mismatch = rpc(process, 4, "tools/call", {"name":"search_references","arguments":{"project_id":"foreign","query":"呼吸法","limit":5}})
                assert mismatch["structuredContent"]["code"] == "project_mismatch"
                invalid = rpc(process, 5, "tools/call", {"name":"search_references","arguments":{"project_id":"reference-check","query":"","limit":5}})
                assert invalid["structuredContent"]["code"] == "invalid_input"
                counts = rpc(process, 6, "tools/call", {"name":"project_status","arguments":{"project_id":"reference-check"}})["structuredContent"]
                assert counts["total"] == counts["translated"] == counts["reviewed"] == counts["shared_tm_entries"] == 0
            finally:
                stop(process)
        missing = subprocess.run(base_command + ["--reference-db", str(root / "missing.sqlite3"), "mcp", "--project-id", "reference-check"], capture_output=True, text=True)
        assert missing.returncode != 0 and "Resolve reference database" in missing.stderr
        assert not (root / "missing.sqlite3").exists()
        wrong_db = subprocess.run(base_command + ["--reference-db", str(root / "project.db"), "mcp", "--project-id", "reference-check"], capture_output=True, text=True)
        assert wrong_db.returncode != 0 and "reference metadata" in wrong_db.stderr
        with sqlite3.connect((root / "project.db").as_uri() + "?mode=ro", uri=True) as conn:
            assert list(conn.iterdump()) == work_before
    assert hashlib.sha256(reference.read_bytes()).hexdigest() == digest
    return {"status":"passed", "reference_sha256":digest, "coverage":coverage,
        "checks":["real HTTP Chinese search", "invalid/oversized queries", "source attribution",
            "placeholder exclusion", "real MCP tool discovery/search", "project isolation", "read-only annotations",
            "missing/wrong database rejected", "project translations/TM unchanged", "reference database hash unchanged"]}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--reference-db", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    result = verify(args.binary.resolve(), args.reference_db.resolve())
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
