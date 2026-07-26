#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CONFIG=${1:-"$ROOT/.test-config"}
BIN=${2:-"$ROOT/target/debug/orchid"}
POLICY="$CONFIG/policies/default.json"
HOOK="$CONFIG/hooks/tool-call-smoke.sh"
WORKDIR="$CONFIG/tool-call-smoke-working-dir"

if [ ! -x "$BIN" ]; then
  echo "building $BIN" >&2
  (cd "$ROOT" && cargo build --bin orchid)
fi

mkdir -p "$CONFIG/policies" "$CONFIG/hooks" "$WORKDIR"
POLICY_BACKUP=$(mktemp)
HOOK_BACKUP=$(mktemp)
POLICY_EXISTED=false
HOOK_EXISTED=false
if [ -f "$POLICY" ]; then cp "$POLICY" "$POLICY_BACKUP"; POLICY_EXISTED=true; fi
if [ -f "$HOOK" ]; then cp "$HOOK" "$HOOK_BACKUP"; HOOK_EXISTED=true; fi
restore() {
  if [ "$POLICY_EXISTED" = true ]; then cp "$POLICY_BACKUP" "$POLICY"; else rm -f "$POLICY"; fi
  if [ "$HOOK_EXISTED" = true ]; then cp "$HOOK_BACKUP" "$HOOK"; else rm -f "$HOOK"; fi
  rm -f "$POLICY_BACKUP" "$HOOK_BACKUP"
}
trap restore EXIT

rm -f "$WORKDIR/direct-marker" "$WORKDIR/hook-marker"
cat > "$POLICY" <<'JSON'
{
  "permissions": {"tools": ["bash"]},
  "hooks": {
    "timeout": 10,
    "events": {
      "on-init": [{"script": "hooks/tool-call-smoke.sh", "mode": "sync"}]
    }
  }
}
JSON
cat > "$HOOK" <<EOF
#!/bin/sh
set -eu
"$BIN" --config "$CONFIG" tool-call --id "\$ORCHID_SESSION_ID" --input '{"call_id":"hook-smoke-call","name":"bash","input":{"cmd":"printf hook > hook-marker"}}'
EOF
chmod 755 "$HOOK"

echo "=== tool-call smoke test ==="
echo "config: $CONFIG"
echo "binary: $BIN"

CREATE=$($BIN --config "$CONFIG" create --working-dir "$WORKDIR")
ID=$(printf '%s\n' "$CREATE" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
test -n "$ID"
echo "created session: $ID"
echo "session purpose: direct CLI tool-call"

echo "=== direct tool-call ==="
$BIN --config "$CONFIG" tool-call --id "$ID" \
  --input '{"call_id":"direct-smoke-call","name":"bash","input":{"cmd":"printf direct > direct-marker"}}'
test "$(cat "$WORKDIR/direct-marker")" = direct

echo "direct session events: $CONFIG/sessions/$ID/events.jsonl"
python3 - "$CONFIG/sessions/$ID/events.jsonl" <<'PY'
import json
import sys

print(json.dumps([json.loads(line) for line in open(sys.argv[1])], indent=2))
PY

CREATE_HOOK=$($BIN --config "$CONFIG" create --working-dir "$WORKDIR")
HOOK_ID=$(printf '%s\n' "$CREATE_HOOK" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
echo "created session: $HOOK_ID"
echo "session purpose: synchronous hook-invoked tool-call"
echo "=== hook-invoked tool-call ==="
$BIN --config "$CONFIG" send --no-run --id "$HOOK_ID" smoke
test "$(cat "$WORKDIR/hook-marker")" = hook

echo "hook session events: $CONFIG/sessions/$HOOK_ID/events.jsonl"
python3 - "$CONFIG/sessions/$HOOK_ID/events.jsonl" <<'PY'
import json
import sys

print(json.dumps([json.loads(line) for line in open(sys.argv[1])], indent=2))
PY

python3 - "$CONFIG" "$ID" "$HOOK_ID" <<'PY'
import json
import sys

root, direct_id, hook_id = sys.argv[1:]
for session_id, calls in ((direct_id, ("direct-smoke-call",)), (hook_id, ("hook-smoke-call",))):
    events = [json.loads(line) for line in open(f"{root}/sessions/{session_id}/events.jsonl")]
    for call_id in calls:
        assert any(e["type"] == "tool_call" and e["calls"][0]["call_id"] == call_id for e in events)
        assert any(e["type"] == "tool_result" and e["call_id"] == call_id for e in events)
print("persisted tool-call/result pairs: ok")
PY

echo "tool-call smoke test: PASS"
