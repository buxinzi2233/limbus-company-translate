"""Acquire immutable public Korean archives and compare them with a verified local snapshot."""
import argparse
from datetime import datetime, timezone
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import time
import urllib.error
import urllib.request
import zipfile


def fetch(url: str) -> bytes:
    for attempt in range(1, 4):
        try:
            with urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent":"LCT-source-audit"}), timeout=60) as response:
                return response.read()
        except (urllib.error.URLError, TimeoutError) as error:
            if attempt == 3:
                raise
            print(json.dumps({"event":"archive_retry","url":url,"attempt":attempt,"error":str(error)}), flush=True)
            time.sleep(attempt)
    raise RuntimeError(f"Archive request exhausted attempts: {url}")


def unpack(raw: bytes) -> dict[str, bytes]:
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        bad = archive.testzip()
        if bad:
            raise ValueError(f"Archive CRC failure: {bad}")
        result: dict[str, bytes] = {}
        for item in archive.infolist():
            parts = PurePosixPath(item.filename).parts
            if ".." in parts or item.filename.startswith("/") or (item.external_attr >> 16) & 0o170000 == 0o120000:
                raise ValueError(f"Unsafe archive member: {item.filename}")
            if not item.is_dir() and item.filename.endswith(".json"):
                name = "/".join(parts[1:])
                if name in result:
                    raise ValueError(f"Duplicate archive path: {name}")
                result[name] = archive.read(item)
        return result


def semantic(raw: bytes) -> bytes:
    return json.dumps(json.loads(raw.decode("utf-8-sig")), ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()


def fingerprint(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def compare(before: dict[str, bytes], after: dict[str, bytes]) -> dict[str, list[str]]:
    both = before.keys() & after.keys()
    return {"added":sorted(after.keys() - before.keys()), "deleted":sorted(before.keys() - after.keys()),
        "modified":sorted(name for name in both if semantic(before[name]) != semantic(after[name])),
        "unchanged":sorted(name for name in both if semantic(before[name]) == semantic(after[name]))}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--current", required=True, type=Path)
    parser.add_argument("--archive-dir", required=True, type=Path)
    parser.add_argument("--before", required=True)
    parser.add_argument("--after", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    args.archive_dir.mkdir(parents=True, exist_ok=True)
    snapshots: dict[str, dict[str, bytes]] = {}
    manifests = []
    for commit in (args.before, args.after):
        if len(commit) != 40 or any(c not in "0123456789abcdef" for c in commit):
            raise ValueError(f"Expected exact Git commit SHA: {commit}")
        url = f"https://codeload.github.com/x1bViolet/Limbus-Localization-Files/zip/{commit}"
        path = args.archive_dir / f"{commit}.zip"
        raw = path.read_bytes() if path.exists() else fetch(url)
        files = unpack(raw)
        if not path.exists():
            path.write_bytes(raw)
        snapshots[commit] = files
        manifests.append({"commit":commit,"url":url,"archive_sha256":fingerprint(raw),"json_files":len(files)})
    local: dict[str, bytes] = {}
    for path in args.current.rglob("*.json"):
        name = "/".join(part.removeprefix("KR_") for part in path.relative_to(args.current).parts)
        if name in local:
            raise ValueError(f"Duplicate normalized current source path: {name}")
        local[name] = path.read_bytes()
    verification = compare(snapshots[args.after], local)
    delta = compare(snapshots[args.before], local)
    report = {"checked_at":datetime.now(timezone.utc).isoformat(),
        "source_kind":"Public community Korean export history; not an official historical manifest",
        "repository":"https://github.com/x1bViolet/Limbus-Localization-Files", "archives":manifests,
        "current_snapshot":str(args.current.resolve()),
        "current_archive_verification":{"counts":{k:len(v) for k,v in verification.items()},"differences":{k:v for k,v in verification.items() if k!='unchanged'}},
        "historical_delta":{"counts":{k:len(v) for k,v in delta.items()},**delta},
        "comparison":"Canonical JSON equality, not raw byte equality; no positional bilingual alignment",
        "current_files":[{"path":name,"sha256":fingerprint(raw)} for name,raw in sorted(local.items())]}
    args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({"archives":manifests,"current_archive_verification":report['current_archive_verification'],"delta_counts":report['historical_delta']['counts']},ensure_ascii=False,indent=2))


if __name__ == "__main__":
    main()
