#!/bin/sh
set -eu

ORCHID_BIN=${ORCHID_BIN:-./bin/orchid}
LM_STUDIO_URL=${LM_STUDIO_URL:-http://localhost:1234}
LM_STUDIO_MODEL=${LM_STUDIO_MODEL:-qwen/qwen3.6-35b-a3b}
SMOKE_EXPECTED=${SMOKE_EXPECTED:-ORCHID_LMSTUDIO_SMOKE_PASS}
SMOKE_TIMEOUT=${SMOKE_TIMEOUT:-120}
KEEP_SMOKE_CONFIG=${KEEP_SMOKE_CONFIG:-1}

fail() {
    printf 'FAIL: %s\n' "$*" >&2
    exit 1
}

command -v jq >/dev/null 2>&1 || fail "jq is required"
[ -x "$ORCHID_BIN" ] || fail "Orchid binary is not executable: $ORCHID_BIN"

SMOKE_CONFIG=$(mktemp -d "${TMPDIR:-/tmp}/orchid-lmstudio-smoke.XXXXXX")
if [ "$KEEP_SMOKE_CONFIG" != "1" ]; then
    trap 'rm -rf "$SMOKE_CONFIG"' EXIT HUP INT TERM
fi

mkdir -p \
    "$SMOKE_CONFIG/agents" \
    "$SMOKE_CONFIG/auth" \
    "$SMOKE_CONFIG/connections" \
    "$SMOKE_CONFIG/policies" \
    "$SMOKE_CONFIG/prompts"

printf '%s\n' '{"log_level":"debug"}' >"$SMOKE_CONFIG/config.json"
printf '%s\n' '{"policy":"default","prompt":"default"}' \
    >"$SMOKE_CONFIG/agents/default.json"
printf '%s\n' '{"max_tokens":4096,"connections":["lmstudio"],"tools":[]}' \
    >"$SMOKE_CONFIG/policies/default.json"
printf '%s\n' 'You are a concise assistant. Follow the requested output exactly.' \
    >"$SMOKE_CONFIG/prompts/default.txt"

jq -n \
    --arg base_url "$LM_STUDIO_URL" \
    --arg model "$LM_STUDIO_MODEL" \
    '{
        interface: "local",
        base_url: $base_url,
        model: $model,
        params: {reasoning_effort: "none", max_tokens: 64}
    }' >"$SMOKE_CONFIG/connections/lmstudio.json"

printf 'Config: %s\n' "$SMOKE_CONFIG"
printf 'Server: %s\n' "$LM_STUDIO_URL"
printf 'Model:  %s\n' "$LM_STUDIO_MODEL"

CREATE_RESULT=$("$ORCHID_BIN" --config "$SMOKE_CONFIG" create --label lmstudio-smoke)
SESSION_ID=$(printf '%s\n' "$CREATE_RESULT" | jq -er '.id') \
    || fail "could not read the created session ID: $CREATE_RESULT"
printf 'Session: %s\n' "$SESSION_ID"

"$ORCHID_BIN" --config "$SMOKE_CONFIG" send --id "$SESSION_ID" \
    "Reply with exactly: $SMOKE_EXPECTED"

AWAIT_RESULT=$("$ORCHID_BIN" --config "$SMOKE_CONFIG" await "$SESSION_ID" \
    --timeout "$SMOKE_TIMEOUT")
STATUS=$(printf '%s\n' "$AWAIT_RESULT" | jq -er \
    --arg id "$SESSION_ID" '.sessions[] | select(.id == $id) | .status') \
    || fail "session did not reach a terminal state: $AWAIT_RESULT"
[ "$STATUS" = "idle" ] || fail "session finished with status '$STATUS'"

GET_RESULT=$("$ORCHID_BIN" --config "$SMOKE_CONFIG" get "$SESSION_ID" --last-message)
ACTUAL=$(printf '%s\n' "$GET_RESULT" | jq -er '.last_message') \
    || fail "could not read the assistant response: $GET_RESULT"
[ "$ACTUAL" = "$SMOKE_EXPECTED" ] \
    || fail "expected '$SMOKE_EXPECTED', received '$ACTUAL'"

printf 'PASS: Orchid received and persisted %s\n' "$ACTUAL"
if [ "$KEEP_SMOKE_CONFIG" = "1" ]; then
    printf 'Session files retained at: %s/sessions/%s\n' "$SMOKE_CONFIG" "$SESSION_ID"
    printf 'Remove them when finished: rm -rf %s\n' "$SMOKE_CONFIG"
else
    printf 'Temporary config and session files will now be removed.\n'
fi
