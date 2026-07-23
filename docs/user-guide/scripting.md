# Scripting

## Await orchestration

`await` observes sessions without changing their state. It treats `idle`, `failed`, and `cancelled` as terminal.

### Launch and collect IDs

```bash
IDS=()
for task in "run tests" "review the diff" "update the docs"; do
  IDS+=("$(orchid send "$task" | jq -r .id)")
done
```

### Await in batches

```bash
orchid await "${IDS[@]}" --timeout 300 --interval 2
```

The result contains every terminal session found in a poll:

```json
{"completed":[{"id":"...","status":"idle"}]}
```

A timeout returns `{"completed":[],"timed_out":true}` and exits with code `2`. Errors are JSON on stderr and exit with code `1`.

### Process results and repeat

Remove completed IDs before awaiting the remaining sessions:

```bash
result=$(orchid await "${IDS[@]}" --timeout 30) || {
  code=$?
  [ "$code" -eq 2 ] || exit "$code"
  result=$(cat /dev/null)
}

mapfile -t done < <(jq -r '.completed[].id' <<<"$result")
for id in "${done[@]}"; do
  jq -r 'select(.type == "message" and .message.role == "assistant") | .message.content' \
    "./config/sessions/$id/conversation.jsonl" | tail -1
done

remaining=()
for id in "${IDS[@]}"; do
  [[ " ${done[*]} " == *" $id "* ]] || remaining+=("$id")
done
[ "${#remaining[@]}" -eq 0 ] || orchid await "${remaining[@]}" --timeout 300
```

`await` only observes session state; it does not stop, kill, or otherwise control sessions. Use [`orchid get`](get.md) to retrieve session data through the selected config boundary. See [sending.md](sending.md) for session setup and [conversations.md](conversations.md) for storage details.

All errors are written as JSON to stderr. Exit code is `0` on success, `1` on an error, and `2` when `await` reaches its timeout.

```json
{"error":"conversation not found: fix-auth-bug"}
```

```bash
if ! orchid send --await "message" 2>/tmp/orchid-err; then
  jq -r .error /tmp/orchid-err
  exit 1
fi
```

## Inspecting session results

Use `get` rather than reading session files directly in scripts:

```bash
ID=$(orchid --config ./config send "run the tests" | jq -r .id)
orchid --config ./config await "$ID" --timeout 600
orchid --config ./config get "$ID" --state --metadata
orchid --config ./config get "$ID" --last-message \\
  | jq -r '.last_message.message.content'
```

Retrieve the final `N` transcript events as a JSON array:

```bash
N=10
orchid --config ./config get "$ID" --conversation \\
  | jq --argjson n "$N" '.conversation | .[-$n:]'
```

Add `[]` to the jq expression to emit one event per line. `get --conversation`
parses JSONL and preserves event order. Reads are point-in-time and read-only,
including for running sessions. See [get.md](get.md) for selectors and errors.

## Patterns

### Fire-and-forget

Dispatch a run and capture the ID for later follow-up:

```bash
ID=$(orchid send "run the audit" | jq -r .id)
```

The run is already in progress. Use the ID to poll or send follow-ups:

```bash
# poll until idle
until [ "$(jq -r .status ./config/sessions/$ID/state.json)" = "idle" ]; do
  sleep 2
done

# read last assistant message
jq -r 'select(.type == "message" and .message.role == "assistant") | .message.content' \
  ./config/sessions/$ID/conversation.jsonl | tail -1
```

### Blocking with `--await`

```bash
ID=$(orchid create | jq -r .id)
orchid set --id $ID --working-dir /path/to/project
orchid send --id $ID --await "fix the failing test" || {
  echo "run failed"
  exit 1
}
```

### Per-project conversation

```bash
# Create and configure once
ID=$(orchid create | jq -r .id)
orchid set --config ./config --id $ID --label my-project --working-dir /path/to/project

# All subsequent sends use the ID
orchid send --id $ID --await "add a readme"
orchid send --id $ID --await "write tests for the new module"
```

Labels are for human reference only — always use the hex ID in scripts.

## jq recipes

Prefer `orchid get` for orchestration scripts. Direct file reads are useful for
local diagnostics and live streaming, but bypass the CLI's config and resource
validation.

```bash
ID=<session-id>
orchid --config ./config get "$ID" --conversation \\
  | jq '.conversation | .[-10:]'                                      # last 10 events
orchid --config ./config get "$ID" --conversation \\
  | jq '.conversation | .[-10:][]'                                   # one event per line
orchid --config ./config get "$ID" --last-message \\
  | jq -r '.last_message.message.content'                             # latest assistant text
```

For direct local inspection:

```bash
FILE=./config/sessions/<id>/conversation.jsonl
jq 'select(.type == "message")' $FILE                                # messages only
jq 'select(.type == "tool_call") | .tool_call.calls[] | {name, input}' $FILE
jq -s '.' $FILE                                                       # full history as array
```
