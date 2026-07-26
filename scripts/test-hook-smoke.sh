#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CONFIG=${1:-"$ROOT/.test-config"}
BIN=${2:-"$ROOT/target/debug/orchid"}

if [ ! -x "$BIN" ]; then
  echo "building $BIN" >&2
  (cd "$ROOT" && cargo build --bin orchid)
fi

echo "using config: $CONFIG"
echo "using binary: $BIN"

HOOK_LOG="$CONFIG/hook-events.jsonl"
: > "$HOOK_LOG"

CREATE=$($BIN --config "$CONFIG" create)
ID=$(printf '%s\n' "$CREATE" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
if [ -z "$ID" ]; then
  echo "failed to extract session id from: $CREATE" >&2
  exit 1
fi

echo "created session: $ID"
printf '%s\n' "$CREATE"

echo "sending hi"
$BIN --config "$CONFIG" send --id "$ID" hi

echo "waiting for worker"
$BIN --config "$CONFIG" await "$ID" --timeout 30

echo "session"
$BIN --config "$CONFIG" get "$ID"

if [ -f "$HOOK_LOG" ]; then
  echo "hook events: $HOOK_LOG"
  tail -n 20 "$HOOK_LOG"
else
  echo "hook log not found: $HOOK_LOG" >&2
  exit 1
fi
