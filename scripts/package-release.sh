#!/usr/bin/env bash
set -euo pipefail
export PYTHONDONTWRITEBYTECODE=1
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
tar --exclude='__pycache__' --exclude='./.git' --exclude='./.loc' --exclude='./target' --exclude='./dist' --exclude='./ui/node_modules' --exclude='./ui/dist' -cf - . | tar -xf - -C "$scratch/source"
cd "$scratch/source"
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui build
CARGO_TARGET_DIR="$scratch/target" cargo build --release --locked -p loc-cli
bundle="lct-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"
mkdir "$scratch/$bundle"
cp "$scratch/target/release/lct" "$scratch/$bundle/lct"
mkdir -p "$scratch/$bundle/docs" "$scratch/$bundle/.agents/skills" "$scratch/$bundle/scripts"
cp README.md README_EN.md LICENSE "$scratch/$bundle/"
cp -R corpus research tools "$scratch/$bundle/"
cp -R docs/images "$scratch/$bundle/docs/images"
cp scripts/*.py scripts/*.sh "$scratch/$bundle/scripts/"
cp docs/AI_AGENT_INTEGRATION.md docs/PRODUCTION_TODO.md docs/UI_ART_DIRECTION.md "$scratch/$bundle/docs/"
cp -R .agents/skills/lct-localization "$scratch/$bundle/.agents/skills/"
(
  cd "$scratch/$bundle"
  find lct docs .agents scripts corpus research tools README.md README_EN.md LICENSE -type f -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS
)
# Run the packaged binary outside its source tree, with a fresh database.
cd "$scratch"
python3 "$scratch/$bundle/scripts/prepare-references.py" "$scratch/references"
python3 "$scratch/$bundle/scripts/verify-references.py" --binary "$scratch/$bundle/lct" --reference-db "$scratch/references/references.sqlite3" --output "$scratch/reference-check.json"
python3 "$scratch/$bundle/scripts/verify-onboarding.py" --binary "$scratch/$bundle/lct" --output "$output/onboarding-verification.json"
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
(
  cd "$output"
  sha256sum "$bundle.tar.gz" > "$bundle.tar.gz.sha256"
)
printf 'Package: %s\n' "$archive"
