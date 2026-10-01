#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 1 ]]; then
  printf 'Usage: %s /absolute/output-directory\n' "$0" >&2
  exit 2
fi
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
output=$(realpath -m -- "$1")
case "$output" in "$PWD"|"$PWD/"*) printf 'Package output must be outside the source checkout\n' >&2; exit 2;; esac
mkdir -p -- "$output"
scratch=$(mktemp -d -t lct-package-XXXXXXXX)
trap 'rm -rf -- "$scratch"' EXIT
mkdir "$scratch/source"
tar --exclude='./.git' --exclude='./.loc' --exclude='./target' --exclude='./ui/node_modules' --exclude='./ui/dist' -cf - . | tar -xf - -C "$scratch/source"
cd "$scratch/source"
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui build
CARGO_TARGET_DIR="$scratch/target" cargo build --release --locked -p loc-cli
bundle="lct-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"
mkdir "$scratch/$bundle"
cp "$scratch/target/release/lct" "$scratch/$bundle/lct"
mkdir -p "$scratch/$bundle/docs" "$scratch/$bundle/.agents/skills"
cp docs/AI_AGENT_INTEGRATION.md docs/PRODUCTION_TODO.md docs/UI_ART_DIRECTION.md "$scratch/$bundle/docs/"
cp -R .agents/skills/lct-localization "$scratch/$bundle/.agents/skills/"
(
  cd "$scratch/$bundle"
  find lct docs .agents -type f -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS
)
# Run the packaged binary outside its source tree, with a fresh database.
cd "$scratch"
"$scratch/$bundle/lct" --data-dir "$scratch/data" init package-check
"$scratch/$bundle/lct" --data-dir "$scratch/data" --json status > "$scratch/status.json"
python3 - "$scratch/status.json" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as handle:
    status = json.load(handle)
assert status["project"]["id"] == "package-check"
PY
archive="$output/$bundle.tar.gz"
if [[ -e "$archive" ]]; then
  printf 'Refusing to overwrite existing package: %s\n' "$archive" >&2
  exit 1
fi
tar -czf "$archive" "$bundle"
sha256sum "$archive" > "$archive.sha256"
printf 'Package: %s\n' "$archive"
