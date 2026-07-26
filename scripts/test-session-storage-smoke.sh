#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CONFIG=${1:-"$ROOT/.test-session-storage-config"}
BIN=${2:-"$ROOT/target/debug/orchid"}

if [ ! -x "$BIN" ]; then
  echo "building $BIN" >&2
  (cd "$ROOT" && cargo build --bin orchid)
fi

WORKDIR="$CONFIG/session-storage-smoke-working-dir"
mkdir -p "$WORKDIR"

echo "=== session storage smoke test ==="
echo "config: $CONFIG"
echo "binary: $BIN"

CREATE=$($BIN --config "$CONFIG" create \
  --label session-storage-smoke \
  --working-dir "$WORKDIR")
ID=$(printf '%s\n' "$CREATE" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
if [ -z "$ID" ]; then
  echo "failed to extract session id from: $CREATE" >&2
  exit 1
fi

SESSION_DIR="$CONFIG/sessions/$ID"
METADATA="$SESSION_DIR/metadata.json"
EVENTS="$SESSION_DIR/events.jsonl"
LOGS="$SESSION_DIR/logs.jsonl"

test -f "$METADATA"
test -f "$EVENTS"
test -f "$LOGS"
test ! -e "$SESSION_DIR/state.json"

python3 - "$METADATA" "$ID" "$WORKDIR" <<'PY'
import json
import sys

metadata = json.load(open(sys.argv[1]))
assert metadata["id"] == sys.argv[2]
assert metadata["label"] == "session-storage-smoke"
assert metadata["working_dir"] == sys.argv[3]
assert metadata["status"] == "idle"
assert metadata["agent"] == "default"
assert "last_message" not in metadata
print(json.dumps(metadata, indent=2))
PY

test ! -s "$EVENTS"

UPDATED=$($BIN --config "$CONFIG" session "$ID" --agent default)
printf '%s\n' "$UPDATED"

GET=$($BIN --config "$CONFIG" get "$ID")
SESSION_JSON=$GET python3 - "$ID" <<'PY'
import json
import os
import sys

session = json.loads(os.environ["SESSION_JSON"])
assert session["metadata"]["id"] == sys.argv[1]
assert session["metadata"]["status"] == "idle"
assert session["events"] == []
assert "last_message" not in session["metadata"]
print("session load: ok")
PY

LIST=$($BIN --config "$CONFIG" list)
LIST_JSON=$LIST python3 - "$ID" <<'PY'
import json
import os
import sys

sessions = json.loads(os.environ["LIST_JSON"])["sessions"]
assert any(session["metadata"]["id"] == sys.argv[1] for session in sessions)
print("session list: ok")
PY

echo "session storage smoke test: PASS ($ID)"
