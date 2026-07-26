#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CONFIG=${1:-"$ROOT/.test-config"}
BIN=${2:-"$ROOT/target/debug/orchid"}

if [ ! -x "$BIN" ]; then
  echo "building $BIN" >&2
  (cd "$ROOT" && cargo build --bin orchid)
fi

echo
echo "=== configuration ==="
echo "config: $CONFIG"
echo "binary: $BIN"

WORKDIR="$CONFIG/smoke-working-dir"
mkdir -p "$WORKDIR"
rm -f "$WORKDIR/hook-events.jsonl"

CREATE=$($BIN --config "$CONFIG" create --working-dir "$WORKDIR")
ID=$(printf '%s\n' "$CREATE" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
if [ -z "$ID" ]; then
  echo "failed to extract session id from: $CREATE" >&2
  exit 1
fi
HOOK_LOG="$CONFIG/sessions/$ID/hook-events.jsonl"

echo
echo "=== create ==="
echo "created session: $ID"
printf '%s\n' "$CREATE"

echo
echo "=== send ==="
$BIN --config "$CONFIG" send --id "$ID" hi

echo
echo "=== await ==="
$BIN --config "$CONFIG" await "$ID" --timeout 30

echo
echo "=== session summary ==="
SESSION=$($BIN --config "$CONFIG" get "$ID")
printf '%s\n' "$SESSION" | python3 -c '
import json, sys
session = json.load(sys.stdin)
print(json.dumps({
    "id": session["metadata"]["id"],
    "status": session["metadata"]["status"],
    "working_dir": session["metadata"]["working_dir"],
    "event_count": len(session["events"]),
    "last_message": next((event["content"] for event in reversed(session["events"]) if event["type"] == "message" and event["role"] == "assistant"), None),
}))
'

if [ -f "$HOOK_LOG" ]; then
  echo
  echo "=== hook invocations ==="
  echo "log: $HOOK_LOG"
  python3 - "$HOOK_LOG" <<'PY'
import json, sys
for index, line in enumerate(open(sys.argv[1]), 1):
    print(f"\n-- hook invocation {index} --")
    print(json.dumps(json.loads(line), indent=2))
PY

  EVENTS="$CONFIG/sessions/$ID/events.jsonl"
  echo
  echo "=== persisted conversation events ==="
  echo "events: $EVENTS"
  python3 - "$EVENTS" <<'PY'
import json, sys
for index, line in enumerate(open(sys.argv[1]), 1):
    print(f"\n-- event {index} --")
    print(json.dumps(json.loads(line), indent=2))
PY
else
  echo "hook log not found: $HOOK_LOG" >&2
  exit 1
fi
