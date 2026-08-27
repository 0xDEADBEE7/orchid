#!/usr/bin/env bash
set -euo pipefail

repo=${1:-.}
repo=$(cd "$repo" && pwd)
stash_dir="$repo/.stash"

if ! command -v stash >/dev/null 2>&1; then
    echo "stash is required to store extracted metadata" >&2
    exit 1
fi

mkdir -p "$stash_dir"
"assay" extract "$repo" | STASH_DIR="$stash_dir" stash set
