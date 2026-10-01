"""Extract scope evidence from catalog-bound official Unity caches without game writes."""
import argparse
import hashlib
import re
from pathlib import Path
from typing import cast

import UnityPy
from json_data import Json, arr, integer, obj, save, text


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def cache_bundle(catalog: bytes, cache: Path, prefix: str) -> tuple[Path, str]:
    names = set(re.findall(re.escape(prefix.encode()) + rb"[0-9a-f]{32}\.bundle", catalog))
    if len(names) != 1:
        raise ValueError(f"Expected one catalog bundle for {prefix}, found {len(names)}")
    name = names.pop().decode()
    identifier = name.removesuffix(".bundle").rsplit("_", 1)[1]
    paths = list(cache.glob(f"*/{identifier}/__data"))
    if len(paths) != 1:
        raise FileNotFoundError(f"Expected one cached {name} under {cache}, found {len(paths)}")
    return paths[0], name


def extract_texts(bundle: Path, output: Path, selected: set[str] | None) -> list[dict[str, str | int]]:
    environment = UnityPy.load(str(bundle))
    rows: list[dict[str, str | int]] = []
    seen: set[str] = set()
    output.mkdir(parents=True, exist_ok=True)
    for asset in environment.objects:
        if asset.type.name != "TextAsset":
            continue
        data = asset.read()
        name = data.m_Name
        if selected is not None and name not in selected:
            continue
        if not isinstance(name, str) or Path(name).name != name or name in seen:
            raise ValueError(f"Unsafe or duplicate TextAsset name: {name!r}")
        seen.add(name)
        payload = data.m_Script.encode("utf-8", "surrogateescape")
        (output / name).write_bytes(payload)
        rows.append({"name": name, "path_id": asset.path_id, "sha256": hashlib.sha256(payload).hexdigest()})
    if selected is not None and seen != selected:
        raise ValueError(f"Missing scenario assets: {sorted(selected - seen)}")
    return sorted(rows, key=lambda row: str(row["name"]))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--resources", type=Path, required=True)
    parser.add_argument("--catalog", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--snapshot", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    resource = UnityPy.load(str(args.resources))
    versions = {file.unity_version for file in resource.files.values()}
    if len(versions) != 1:
        raise ValueError(f"Ambiguous installed Unity versions: {versions}")
    version = versions.pop()
    # Bundles strip the version; use the actual installed serialized-file header.
    UnityPy.config.FALLBACK_UNITY_VERSION = version
    catalog = args.catalog.read_bytes()
    inventory: list[dict[str, object]] = []
    for prefix, directory, selected in [
        ("static_s1_0_assets_all_", "static", None),
        ("storyeffect_s1_0_assets_all_", "scenarios", {f"S{n}B" for n in range(1017, 1030)}),
    ]:
        bundle, name = cache_bundle(catalog, args.cache, prefix)
        rows = extract_texts(bundle, args.snapshot / directory, selected)
        inventory.append({"bundle": name, "cache_path": str(bundle), "sha256": sha(bundle), "directory": directory, "assets": rows})
    bundle, name = cache_bundle(catalog, args.cache, "s8_a1c10p2_produce_assets_all_")
    environment = UnityPy.load(str(bundle))
    objects = {asset.path_id: asset for asset in environment.objects}
    matches: list[dict[str, object]] = []
    for asset in environment.objects:
        if asset.type.name != "MonoBehaviour" or b"S9992B" not in asset.get_raw_data():
            continue
        tree = obj(cast(Json, asset.read_typetree()))
        if tree.get("storyId") != "S9992B":
            raise ValueError("S9992B bytes were not a typed storyId reference")
        go_id = integer(obj(tree["m_GameObject"])["m_PathID"])
        go = obj(cast(Json, objects[go_id].read_typetree()))
        if text(go["m_Name"]) != "ProduceObject_a1c10p2_Last":
            raise ValueError(f"Unexpected ending caller: {go['m_Name']}")
        if asset.path_id not in [integer(obj(obj(item)["component"])["m_PathID"]) for item in arr(go["m_Component"])]:
            raise ValueError("Ending component is not attached to the named GameObject")
        containers = [key for key, value in environment.container.items() if value.path_id == go_id]
        if not containers:
            raise ValueError("Ending GameObject is not a catalog prefab root")
        matches.append({"component_path_id": asset.path_id, "component": tree, "game_object_path_id": go_id, "game_object": go, "prefab_paths": containers})
    if len(matches) != 1:
        raise ValueError(f"Expected one typed S9992B ending caller, found {len(matches)}")
    save(args.evidence / "ending-produce.json", {"bundle": name, "origin": str(bundle), "sha256": sha(bundle), "matches": matches})
    save(args.evidence / "asset-verification.json", {
        "resources": str(args.resources), "resources_sha256": sha(args.resources),
        "unity_version_from_resources_header": version,
        "catalog": str(args.catalog), "catalog_sha256": sha(args.catalog),
        "bundle_binding": "Exact name in current local catalog and exact cache version directory; not a remote freshness or runtime playback claim",
        "bundles": inventory, "game_files_written": False,
    })
    print({"unity_version": version, "text_assets": sum(len(row["assets"]) for row in inventory), "ending_callers": len(matches)})


if __name__ == "__main__":
    main()
