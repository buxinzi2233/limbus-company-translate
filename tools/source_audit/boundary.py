"""Partition the verified Part2 update into explicit translation and exclusion work orders."""
import argparse
from collections import Counter
from dataclasses import asdict, dataclass
import hashlib
import json
from pathlib import Path
import re
import sqlite3

from json_data import Json, arr, integer, leaves, load, obj, save, text


@dataclass(frozen=True)
class Decision:
    path: str
    source_identity: str
    kind: str
    pointer: str | None
    disposition: str
    reason: str
    evidence: tuple[str, ...]


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def record_id(identity: str) -> str | int:
    parts = arr(json.loads(identity))
    entry = obj(obj(parts[1])["Entry"])
    value = entry["value"]
    if type(value) not in (str, int):
        raise TypeError(f"Invalid record ID: {value!r}")
    return value


def source_file(root: Path, path: str) -> Path:
    relative = Path(path)
    return root / "sources/client-20260926/kr" / relative.parent / ("KR_" + relative.name)


def pointer_value(value: Json, pointer: str) -> Json:
    for token in pointer.split("/")[1:]:
        token = token.replace("~1", "/").replace("~0", "~")
        value = arr(value)[int(token)] if isinstance(value, list) else obj(value)[token]
    return value


def routes(root: Path) -> dict[str, tuple[str, ...]]:
    index = obj(load(root / "sources/client-20260926/indices/rpg-loc-file-list-a1c10p2.json"))
    result: dict[str, tuple[str, ...]] = {}
    for values in index.values():
        for value in arr(values):
            row = obj(value)
            name = "RPGSystem/" + text(row["file"]) + ".json"
            if name in result:
                raise ValueError(f"Duplicate RPG index entry: {name}")
            result[name] = tuple(text(route) for route in arr(row["routes"])) if "routes" in row else ("A", "B")
    return result


def dependency_evidence(root: Path, core: set[str]) -> dict[str, Json]:
    static = root / "sources/client-20260926/static"
    models: dict[str, Json] = {}
    story_calls: dict[str, Json] = {}
    for number in range(1017, 1030):
        story = f"S{number}B"
        metadata = obj(load(root / "sources/client-20260926/scenarios" / story))
        for i, value in enumerate(arr(metadata["dataList"])):
            row = obj(value)
            if "characterlist" not in row:
                continue
            for item in re.split(r"[;|]", text(row["characterlist"])):
                model = item.split(":")[0].strip()
                previous = arr(models[model]) if model in models else []
                models[model] = [*previous, f"scenarios/{story}:/dataList/{i}/characterlist"]
        hits: list[Json] = []
        for path in sorted(static.glob("rpg-cutscene-data-*-b")):
            for pointer, value in leaves(load(path), ""):
                if value == story:
                    hits.append(f"static/{path.name}:{pointer}")
        config_hits = [f"static/story-config-a1c10p2:{pointer}" for pointer, value in leaves(load(static / "story-config-a1c10p2"), "") if value == story]
        if len(config_hits) != 1:
            raise ValueError(f"Expected one explicit Part2 story config entry for {story}")
        story_calls[story] = {"chapter_config": config_hits, "cutscene_callers": hits, "runtime_reachability_proven": False}
    keywords: dict[str, Json] = {}
    for path in sorted(core):
        for pointer, value in leaves(load(source_file(root, path)), ""):
            if not isinstance(value, str):
                continue
            for token in re.findall(r"\[([A-Za-z][A-Za-z0-9_]*)\]", value):
                previous = arr(keywords[token]) if token in keywords else []
                keywords[token] = [*previous, f"kr/{path}:{pointer}"]
    battle_ids: set[int] = set()
    for path in sorted(static.glob("rpg-*")):
        if path.name.endswith("-b") or "-b-" in path.name or "a1c10p2" in path.name:
            for pointer, value in leaves(load(path), ""):
                if pointer.rsplit("/", 1)[-1].lower() == "battleid" and type(value) is int and value > 0:
                    battle_ids.add(value)
    battles: dict[int, Json] = {}
    units: dict[int, Json] = {}
    for part in (1, 2):
        for value in arr(obj(load(static / f"storybattle-a1c10p{part}"))["list"]):
            row = obj(value)
            battles[integer(row["id"])] = row
            if part == 2:
                battle_ids.add(integer(row["id"]))
        for kind in ("abnormality-unit", "enemy"):
            for value in arr(obj(load(static / f"{kind}-a1c10p{part}"))["list"]):
                row = obj(value)
                units[integer(row["id"])] = row
    unit_ids = {integer(value) for battle in battle_ids for pointer, value in leaves(battles[battle], "") if pointer.endswith("/unitID")}
    panic_ids = {integer(obj(units[unit])["panicType"]) for unit in unit_ids if "panicType" in obj(units[unit])}
    skill_hits = [f"static/abnormality-unit-a1c10p2:{pointer}" for pointer, value in leaves(load(static / "abnormality-unit-a1c10p2"), "") if pointer.lower().endswith("/skillid") and value == 149607]
    if not skill_hits:
        raise ValueError("Missing reused Part1 skill 149607 evidence")
    return {"story_calls": story_calls, "model_calls": models, "keyword_calls": keywords,
            "battle_ids": sorted(battle_ids), "unit_ids": sorted(unit_ids), "panic_ids": sorted(panic_ids),
            "reused_skill_149607": skill_hits}


def classify(row: dict[str, Json], core: set[str], contemporary: set[str], route_index: dict[str, tuple[str, ...]], deps: dict[str, Json]) -> Decision:
    path, kind, identity = text(row["path"]), text(row["kind"]), text(row["source_identity"])
    pointer = text(row["new_pointer"]) if row["new_pointer"] is not None else None
    if row["identity_scope"] == "snapshot_bound":
        return Decision(path, identity, kind, pointer, "identity_rebind_only", "S1004B has one positional spelling change, not 384 new translations", ("data/remaining-todo/semantic-review.json",))
    if path in core:
        return Decision(path, identity, kind, pointer, "part2_core", "Verified new Part2 story, B-route RPG, or Part2 battle resource", ("data/part2-boundary/dependencies.json",))
    if path in contemporary:
        return Decision(path, identity, kind, pointer, "excluded_contemporary", "Rodion 10917 personality story or voice, not the mainline", ("data/translation-scope.json",))
    rid = record_id(identity)
    if path in route_index:
        route_set = route_index[path]
        disposition = "excluded_route_a" if "B" not in route_set else "part2_shared"
        return Decision(path, identity, kind, pointer, disposition, f"Official RPG localization routes: {','.join(route_set)}", ("sources/client-20260926/indices/rpg-loc-file-list-a1c10p2.json",))
    if path in ("BattleKeywords.json", "Bufs.json"):
        hits = obj(deps["keyword_calls"])
        evidence = tuple(text(v) for v in arr(hits[str(rid)])) if str(rid) in hits else ()
        return Decision(path, identity, kind, pointer, "part2_shared" if evidence else "excluded_no_part2_dependency", "Exact bracket-token references in Part2 resources; no whole-file inclusion", evidence)
    if path == "ScenarioModelCodes-AutoCreated.json":
        hits = obj(deps["model_calls"])
        evidence = tuple(text(v) for v in arr(hits[str(rid)])) if str(rid) in hits else ()
        return Decision(path, identity, kind, pointer, "part2_shared" if evidence else "excluded_no_part2_dependency", "Exact characterlist model identifiers in all 13 Part2 story-effect assets; no substring matching", evidence)
    if path == "PanicInfo-a1c10p1.json":
        needed = rid in arr(deps["panic_ids"])
        return Decision(path, identity, kind, pointer, "part2_shared" if needed else "excluded_no_part2_dependency", "Panic types of units in B-route/Part2 battle tables", ("data/part2-boundary/dependencies.json",))
    if path == "Skills_Abnormality-a1c10p1.json":
        if rid != 149607:
            raise ValueError(f"Unexpected changed shared skill: {rid}")
        return Decision(path, identity, kind, pointer, "part2_shared", "Part2 abnormality unit reuses this skill ID", tuple(text(v) for v in arr(deps["reused_skill_149607"])))
    if path == "RPGSuicideBoxUI.json":
        return Decision(path, identity, kind, pointer, "part2_shared", "Shared RPG reset UI compatibility correction, not Part1 narrative retranslation", ("data/remaining-todo/semantic-review.json",))
    if path == "StoryTheaterUIText.json":
        if rid != "B":
            raise ValueError(f"Unexpected theater label: {rid}")
        return Decision(path, identity, kind, pointer, "part2_shared", "B-route theater label; preserve literal B", ("static/storytheater-main-a1c10p1:/list/0/storyList/4/rpgDetailInfo/storyTitle",))
    if path == "UnitKeyword.json":
        return Decision(path, identity, kind, pointer, "excluded_no_part2_dependency", "UnitKeyword_LE_KHAKI has no reference in the audited Part2 resource dependency set", ())
    excluded = {
        "CouponUIText.json", "EGOVoiceDig/Voice_EGO_HongLu_6.json", "EGOVoiceDig/Voice_EGO_Rodion_9.json",
        "GachaTitle.json", "IAPProduct-a1c10.json", "IntroductionPreset.json", "MirrorDungeonRentalName.json",
        "Passive_Ego.json", "Passives.json", "Personalities.json", "Personality_Get_Condition.json",
        "Skills_Ego_Personality-06.json", "Skills_Ego_Personality-09.json", "Skills_personality-09.json",
        "StoryData/P10416.json", "StoryData/P10816.json", "StoryTheaterMirrorWorldStoryTitle.json",
    }
    if path not in excluded:
        raise ValueError(f"Unclassified changed file: {path}")
    return Decision(path, identity, kind, pointer, "excluded_contemporary", "Personality/EGO, shop/profile, mirror dungeon, or other personality story update; not chapter work", ("data/todo-completion/field-delta.json",))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    args = parser.parse_args()
    root = args.root.resolve()
    output = root / "data/part2-boundary"
    scope = obj(load(root / "data/translation-scope.json"))
    candidates = {text(obj(row)["path"]) for row in arr(scope["core_candidate_files"])}
    contemporary = {text(obj(row)["path"]) for row in arr(scope["contemporary_files_deferred_from_mainline"])}
    route_index = routes(root)
    core = {path for path in candidates if path not in route_index or "B" in route_index[path]}
    deps = dependency_evidence(root, core)
    save(output / "dependencies.json", deps)
    delta = obj(load(root / "data/todo-completion/field-delta.json"))
    decisions = [classify(obj(row), core, contemporary, route_index, deps) for row in arr(delta["changes"])]
    selected = {(row.path, row.source_identity): row for row in decisions if row.disposition.startswith("part2_") and row.kind != "removed"}
    db = root / ".loc/project.db"
    with sqlite3.connect(db.as_uri() + "?mode=ro", uri=True) as conn:
        records = conn.execute('''SELECT s.id,f.relative_path,s.source_identity,s.key_path,s.source_kr,s.updated_at
            FROM segments s JOIN files f ON f.id=s.file_id WHERE s.project_id=? AND s.active=1 AND f.active=1 ORDER BY s.id''', ("limbus-part2-new",)).fetchall()
        before_hash = hashlib.sha256()
        for row in conn.execute("SELECT * FROM segments ORDER BY id"):
            before_hash.update(json.dumps(row, ensure_ascii=False).encode())
    baseline = obj(load(output / "baseline.json"))
    if before_hash.hexdigest() != text(baseline["segment_sha256"]):
        raise ValueError("Production segments changed during boundary audit")
    work_order: list[dict[str, object]] = []
    found: set[tuple[str, str]] = set()
    sources = {path: load(source_file(root, path)) for path in {key[0] for key in selected}}
    for sid, path, identity, pointer, source, version in records:
        key = (path, identity)
        if key not in selected:
            continue
        if pointer_value(sources[path], pointer) != source:
            raise ValueError(f"Persisted source differs from snapshot: {path}:{pointer}")
        decision = selected[key]
        found.add(key)
        action = "translate_or_review_korean" if re.search(r"[가-힣]", source) else "preserve_or_review_non_korean"
        work_order.append({"segment_id": sid, "path": path, "pointer": pointer, "source_identity": identity,
                           "source_kr": source, "observed_updated_at": version, "scope": decision.disposition, "action": action})
    if found != selected.keys():
        raise ValueError(f"Selected identities absent from production: {selected.keys() - found}")
    per_file = [dict(path=path, sha256=digest(source_file(root, path)), selected_segments=sum(row['path'] == path for row in work_order)) for path in sorted(core | {row.path for row in selected.values()})]
    save(output / "decisions.json", [asdict(row) for row in decisions])
    save(output / "work-order.json", {"project_id": "limbus-part2-new", "translation_authorized": False,
        "version_policy": "Re-read through MCP before any authorized draft submission; these versions are evidence only", "files": per_file, "segments": work_order})
    summary = {
        "project_id": "limbus-part2-new", "boundary_status": "confirmed_for_verified_snapshot",
        "boundary_definition": "Part2 new mainline resources plus changed shared records directly needed by B-route, excluding Part1-only and contemporary updates; not a full-client retranslation",
        "initial_candidate_files": len(candidates), "excluded_new_route_a_files": sorted(candidates - core),
        "core_files": len(core), "core_segments": sum(row['scope'] == 'part2_core' for row in work_order),
        "shared_files": len({row['path'] for row in work_order if row['scope'] == 'part2_shared'}),
        "shared_segments": sum(row['scope'] == 'part2_shared' for row in work_order),
        "work_order_files": len(per_file), "files_with_segments": len({row["path"] for row in work_order}),
        "empty_core_files": sorted(core - {str(row["path"]) for row in work_order}), "work_order_segments": len(work_order),
        "actions": dict(Counter(str(row['action']) for row in work_order)),
        "delta_decisions": dict(Counter(row.disposition for row in decisions)),
        "decision_kind_counts": dict(Counter(row.disposition + ':' + row.kind for row in decisions)),
        "full_source_files": 2277, "full_source_segments": len(records), "production_segment_sha256": before_hash.hexdigest(),
        "source_snapshot": "sources/client-20260926/source-verification.json",
        "ending_evidence": "data/part2-boundary/ending-produce.json", "asset_evidence": "data/part2-boundary/asset-verification.json",
        "excluded_future": "Ending C/a1c10p3 has a future release gate; not part of this Part2 snapshot work order",
        "part1_spelling": "S1004B /dataList/226/content: 주의 -> 주위. Separate Part1 maintenance, not 384 new Part2 translations",
        "approvals": "Seven name links confirmed by user separately; no translation approval or write authorization implied",
        "runtime_validation": False,
    }
    save(output / "summary.json", summary)
    print(json.dumps(summary, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
