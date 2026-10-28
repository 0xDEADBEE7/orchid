#!/bin/sh
# on-turn-end: report a finished turn to the session that created this one
# (e.g. an orchestrator), which queues it or starts a turn to react.
input=$(cat)
parent=$(printf '%s' "$input" | jq -r '.session.metadata.parent // empty')
[ -n "$parent" ] || exit 0
note=$(printf '%s' "$input" | jq -r '.session.metadata as $m
  | "<subagent id=\"\($m.id)\" status=\"\($m.status)\">\(.event.content // .event.message // .event.reason // "")</subagent>"')
"${ORCHID_BIN:-orchid}" send --id "$parent" -- "$note" >/dev/null
