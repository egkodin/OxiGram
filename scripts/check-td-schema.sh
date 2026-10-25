#!/usr/bin/env bash
# Verifies that crates/td-api/tl/td_api.tl is TDLib's schema at tdlib/COMMIT.
#
# Usage: scripts/check-td-schema.sh
#   TD_SOURCE   optional path to a tdlib/td git checkout to read from (read only);
#               without it the schema is downloaded from GitHub.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
commit="$(tr -d '[:space:]' < "$repo_root/tdlib/COMMIT")"
vendored="$repo_root/crates/td-api/tl/td_api.tl"
schema_path="td/generate/scheme/td_api.tl"

expected="$(mktemp)"
trap 'rm -f "$expected"' EXIT
if [[ -n "${TD_SOURCE:-}" ]]; then
  git -C "$TD_SOURCE" show "$commit:$schema_path" > "$expected"
else
  curl --fail --silent --show-error --location \
    "https://raw.githubusercontent.com/tdlib/td/$commit/$schema_path" > "$expected"
fi

if ! cmp --silent "$expected" "$vendored"; then
  echo "crates/td-api/tl/td_api.tl differs from $schema_path at ${commit:0:9}" >&2
  exit 1
fi
echo "td_api.tl matches TDLib ${commit:0:9}"
