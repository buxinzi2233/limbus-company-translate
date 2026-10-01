"""Strict JSON accessors for read-only source-boundary audits."""
import json
from pathlib import Path
from typing import cast

type Json = None | bool | int | float | str | list[Json] | dict[str, Json]


def load(path: Path) -> Json:
    return cast(Json, json.loads(path.read_text(encoding="utf-8-sig")))


def obj(value: Json) -> dict[str, Json]:
    if not isinstance(value, dict):
        raise TypeError(f"Expected JSON object, got {type(value).__name__}")
    return value


def arr(value: Json) -> list[Json]:
    if not isinstance(value, list):
        raise TypeError(f"Expected JSON array, got {type(value).__name__}")
    return value


def text(value: Json) -> str:
    if not isinstance(value, str):
        raise TypeError(f"Expected string, got {type(value).__name__}")
    return value


def integer(value: Json) -> int:
    if type(value) is not int:
        raise TypeError(f"Expected integer, got {type(value).__name__}")
    return value


def leaves(value: Json, pointer: str) -> list[tuple[str, Json]]:
    if isinstance(value, dict):
        return [leaf for k, v in value.items() for leaf in leaves(v, pointer + "/" + k.replace("~", "~0").replace("/", "~1"))]
    if isinstance(value, list):
        return [leaf for i, v in enumerate(value) for leaf in leaves(v, pointer + "/" + str(i))]
    return [(pointer, value)]


def save(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")
