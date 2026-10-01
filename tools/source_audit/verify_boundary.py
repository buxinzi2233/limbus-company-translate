"""Integration verification of real source, Unity evidence, work order, and untouched LCT state."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import sqlite3

from json_data import arr, integer, leaves, load, obj, save, text
from boundary import digest, pointer_value, routes, source_file


def require(condition: bool, reason: str) -> None:
    if not condition:
        raise ValueError(reason)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    args = parser.parse_args()
    root = args.root.resolve()
    out = root / "data/part2-boundary"
    snapshot = root / "sources/client-20260926"
    source = obj(load(snapshot / "source-verification.json"))
    original = [obj(row) for row in arr(source["files"])]
    expected_files = {text(row["path"]) for row in original}
    actual_files = {str(path.relative_to(snapshot / "kr")) for path in (snapshot / "kr").rglob("*.json")}
    require(actual_files == expected_files, "Korean snapshot file set changed")
    for row in original:
        require(digest(snapshot / "kr" / text(row["path"])) == text(row["sha256"]), f"Snapshot hash changed: {row['path']}")
        require(digest(Path(text(row["origin"]))) == text(row["sha256"]), f"Installed source changed: {row['origin']}")
    assets = obj(load(out / "asset-verification.json"))
    require(digest(Path(text(assets["resources"]))) == text(source["client_resources_assets_sha256"]), "Installed resources.assets differs from verified source version")
    require(digest(Path(text(assets["catalog"]))) == text(assets["catalog_sha256"]), "Cached catalog changed")
    extracted = 0
    for value in arr(assets["bundles"]):
        bundle = obj(value)
        require(digest(Path(text(bundle["cache_path"]))) == text(bundle["sha256"]), f"Cached bundle changed: {bundle['bundle']}")
        for value in arr(bundle["assets"]):
            asset = obj(value)
            require(digest(snapshot / text(bundle["directory"]) / text(asset["name"])) == text(asset["sha256"]), f"Extracted asset changed: {asset['name']}")
            extracted += 1
    ending = obj(load(out / "ending-produce.json"))
    require(digest(Path(text(ending["origin"]))) == text(ending["sha256"]), "Ending bundle changed")
    match, = arr(ending["matches"])
    component = obj(obj(match)["component"])
    require(component["storyId"] == "S9992B" and component["playStoryOnProduceStart"] == 1, "Ending story binding invalid")
    config = obj(load(snapshot / "static/rpg-all-ending-data-a1c10p2"))
    ending_b, = [obj(row) for row in arr(config["list"]) if obj(row)["key"] == "B"]
    require(ending_b["produceName"] == "a1c10p2_Last", "B ending production binding changed")
    core_scope = obj(load(root / "data/translation-scope.json"))
    core = [obj(row) for row in arr(core_scope["core_candidate_files"])]
    require(len(core) == 98, "Expected 98 core resource files")
    route_index = routes(root)
    excluded_core = {text(row["path"]) for row in core if text(row["path"]) in route_index and "B" not in route_index[text(row["path"]) ]}
    require(len(excluded_core) == 7, "Expected exactly seven newly added A-only candidates")
    rpg_resources = [text(row['path']) for row in core if text(row['path']).startswith('RPGSystem/') and text(row['path']) not in excluded_core]
    require(len(rpg_resources) == 65 and all('B' in route_index[path] for path in rpg_resources), "Core RPG files do not all belong to the official B/common index")
    for number in range(1017, 1030):
        name = f"S{number}B"
        metadata = obj(load(snapshot / "scenarios" / name))
        localized = obj(load(source_file(root, "StoryData/" + name + ".json")))
        metadata_ids = [obj(row)['id'] for row in arr(metadata['dataList'])]
        localized_ids = [obj(row)['id'] for row in arr(localized['dataList'])]
        require(metadata_ids == localized_ids, f"Story effect and localization row IDs differ: {name}")
    work = obj(load(out / "work-order.json"))
    rows = [obj(row) for row in arr(work["segments"])]
    require(work["translation_authorized"] is False, "Scope audit cannot grant translation permission")
    counts = Counter(text(row["path"]) for row in rows)
    for file in core:
        require(counts[text(file["path"])] == (0 if text(file["path"]) in excluded_core else integer(file["expected_segments"])), f"Core field count mismatch: {file['path']}")
    selected_ids = {integer(row["segment_id"]) for row in rows}
    require(len(selected_ids) == len(rows), "Duplicate selected source segment")
    deltas = [obj(row) for row in arr(obj(load(root / "data/todo-completion/field-delta.json"))["changes"])]
    decisions = [obj(row) for row in arr(load(out / "decisions.json"))]
    key = lambda row: (text(row["path"]), text(row["source_identity"]), text(row["kind"]))
    require(Counter(map(key, deltas)) == Counter(map(key, decisions)), "Delta decisions are not an exact complete partition")
    expected_keys = {(text(row["path"]), text(row["source_identity"])) for row in decisions if text(row["disposition"]).startswith("part2_") and row["kind"] != "removed"}
    require(expected_keys == {(text(row["path"]), text(row["source_identity"])) for row in rows}, "Work order includes excluded or misses included fields")
    source_json = {path: load(source_file(root, path)) for path in counts}
    db = root / ".loc/project.db"
    with sqlite3.connect(db.as_uri() + "?mode=ro", uri=True) as conn:
        conn.row_factory = sqlite3.Row
        live = {r['id']: r for r in conn.execute("SELECT * FROM segments WHERE project_id=? AND active=1", ("limbus-part2-new",))}
        for row in rows:
            segment = live[integer(row["segment_id"])]
            require(segment['source_identity'] == row['source_identity'] and segment['source_kr'] == row['source_kr'], f"Live identity/source mismatch: {row['segment_id']}")
            require(pointer_value(source_json[text(row['path'])], text(row['pointer'])) == row['source_kr'], f"Source pointer mismatch: {row['segment_id']}")
        h = hashlib.sha256()
        for row in conn.execute("SELECT * FROM segments ORDER BY id"):
            h.update(json.dumps(tuple(row), ensure_ascii=False).encode())
        require(h.hexdigest() == text(obj(load(out / "baseline.json"))["segment_sha256"]), "Production segment rows changed")
        active_files = conn.execute("SELECT count(*) FROM files WHERE project_id=? AND active=1", ("limbus-part2-new",)).fetchone()[0]
    require(active_files == 2277 and len(live) == 182784, "Production full-snapshot counts changed")
    before = load(root / "sources/history-korean/before-part2/kr/StoryData/S1004B.json")
    after = load(source_file(root, "StoryData/S1004B.json"))
    before_leaves, after_leaves = dict(leaves(before, "")), dict(leaves(after, ""))
    require(before_leaves.keys() == after_leaves.keys(), "S1004B structural keys differ")
    changed = [path for path in before_leaves if before_leaves[path] != after_leaves[path]]
    require(changed == ["/dataList/226/content"], f"Unexpected S1004B changes: {changed}")
    confirmation = obj(load(root / "data/remaining-todo/term-review.json"))
    require(confirmation['human_approved'] is True and len(arr(confirmation['pairs'])) == 7, "Exact seven-name user confirmation missing")
    require(all(obj(row)['review_status'] == 'human_confirmed_name_only' for row in arr(confirmation['pairs'])), "Name confirmation scope mismatch")
    summary = obj(load(out / "summary.json"))
    require(integer(summary['work_order_segments']) == len(rows), "Summary denominator mismatch")
    require(integer(summary['work_order_files']) == len(arr(work['files'])) == 101, "Resource file count mismatch")
    save(out / "verification.json", {
        "passed": True, "verified_korean_files": len(original), "installed_korean_files_unchanged": len(original),
        "verified_extracted_assets": extracted, "official_B_or_common_RPG_files": len(rpg_resources),
        "story_effect_localization_ID_sequences_matched": 13, "classified_delta_fields": len(decisions),
        "selected_segments": len(rows), "work_order_resources": 101, "files_with_selected_segments": len(counts),
        "production_active_files": active_files, "production_active_segments": len(live),
        "production_segment_sha256": h.hexdigest(), "production_segment_rows_unchanged": True,
        "S1004B_actual_changed_pointers": changed, "seven_name_links_human_confirmed": True,
        "scope_is_snapshot_specific": True, "game_runtime_tested": False,
        "work_order_sha256": digest(out / "work-order.json"), "decisions_sha256": digest(out / "decisions.json"),
    })
    print((out / "verification.json").read_text())


if __name__ == "__main__":
    main()
