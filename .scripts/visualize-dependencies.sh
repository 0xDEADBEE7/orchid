#!/usr/bin/env bash
set -euo pipefail

stash_bin=${STASH_BIN:-stash}
stash_dir=${STASH_DIR:-.stash}
fig_bin=${FIG_BIN:-fig}

command -v jq >/dev/null 2>&1 || { echo "jq is required" >&2; exit 1; }
command -v "$stash_bin" >/dev/null 2>&1 || { echo "stash is required" >&2; exit 1; }
command -v "$fig_bin" >/dev/null 2>&1 || { echo "fig is required" >&2; exit 1; }

files=$(STASH_DIR="$stash_dir" "$stash_bin" get type=file | jq -s '.')
imports=$(STASH_DIR="$stash_dir" "$stash_bin" get type=import | jq -s '.')

figure=$(mktemp "${TMPDIR:-/tmp}/assay-dependencies.XXXXXX.json")
trap 'rm -f "$figure"' EXIT

jq -n --argjson files "$files" --argjson imports "$imports" '
  ($files | map(select(.path)) | unique_by(.id)) as $files
  | ($imports | map(select(.path and .resolved_path))
      | map({from: .path, to: .resolved_path}) | unique) as $edges
  | {type: "graph", nodes: ($files | map({id: .path, label: .path})), edges: $edges}
' > "$figure"

"$fig_bin" "$figure"
