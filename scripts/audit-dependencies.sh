#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
tool_root="$PWD/.loc/tools"
audit_version=0.22.2
if [[ ! -x "$tool_root/bin/cargo-audit" ]] || [[ "$("$tool_root/bin/cargo-audit" --version)" != "cargo-audit $audit_version" ]]; then
  cargo install cargo-audit --version "=$audit_version" --locked --root "$tool_root"
fi
"$tool_root/bin/cargo-audit" audit --file Cargo.lock --deny warnings
pnpm --dir ui audit
