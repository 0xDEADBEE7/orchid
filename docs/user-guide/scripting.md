# Scripting

## Await orchestration

`await` observes sessions without changing their state. It treats `idle`, `failed`, and `cancelled` as terminal.

### Launch and collect IDs

```bash
IDS=()
for task in "run tests" "review the diff" "update the docs"; do
  ID=$(orchid --config ./config create | jq -r '.id')
  orchid --config ./config send --id "$ID" "$task"
  IDS+=("$ID")
done
```

### Await in batches

```bash
orchid --config ./config await "${IDS[@]}" --timeout 300
```

The result contains a `sessions` array. Each requested session is included
when it reaches a terminal state:

```json
{"sessions":[{"id":"...","status":"idle"}]}
```

A timeout returns the sessions observed so far. Check the process exit status if
your script must distinguish a timeout from a completed batch.

### Process results and repeat

Remove completed IDs before awaiting the remaining sessions:

```bash
result=$(orchid --config ./config await "${IDS[@]}" --timeout 300)

while read -r id; do
  orchid --config ./config get "$id" --last-message \
    | jq -r '.last_message // empty'
done < <(jq -r '.sessions[] | select(.status != "running") | .id' <<<"$result")
```

`await` only observes session state; it does not stop, kill, or otherwise
control sessions. Use [`orchid get`](get.md) to retrieve session data through
the selected config boundary. Errors are JSON on stderr with exit code `1`.

```json
{"error":"conversation not found: fix-auth-bug"}
```

```bash
ID=$(orchid --config ./config create | jq -r '.id')
orchid --config ./config send --id "$ID" "message"
if ! orchid --config ./config await "$ID" --timeout 600; then
  echo "await failed or timed out" >&2
  exit 1
fi
orchid --config ./config get "$ID" --last-message \
  | jq -r '.last_message // empty'
```

## Inspecting session results

Use `get` rather than reading session files directly in scripts:

```bash
ID=$(orchid --config ./config create | jq -r '.id')
orchid --config ./config send --id "$ID" "run the tests"
orchid --config ./config await "$ID" --timeout 600
orchid --config ./config get "$ID" --state --metadata
orchid --config ./config get "$ID" --last-message \
  | jq -r '.last_message // empty'
```

Retrieve the final `N` transcript events as a JSON array:

```bash
N=10
orchid --config ./config get "$ID" --conversation \\
  | jq --argjson n "$N" '.events | .[-$n:]'
```

To inspect only the most recent event:

```bash
orchid --config ./config get "$ID" --conversation \\
  | jq '.events[-1:]'
```

The result is an array so the command remains safe when the conversation is
empty. Add `[]` to emit one event per line. `get --conversation` parses JSONL
and preserves event order. Reads are point-in-time and read-only, including for
running sessions. See [get.md](get.md) for selectors and errors.

## Patterns

### Fire-and-forget

Dispatch a run and capture the ID for later follow-up:

```bash
ID=$(orchid send "run the audit" | jq -r .id)
```

The run is already in progress. Observe it without sending another message:

```bash
orchid --config ./config await "$ID" --timeout 600
orchid --config ./config get "$ID" --last-message \\
  | jq -r '.last_message // empty'
```
```bash
# observe the run without sending another message
orchid --config ./config await "$ID" --timeout 600
orchid --config ./config get "$ID" --last-message \\
  | jq -r '.last_message // empty'
```

### Blocking with `--await`

```bash
ID=$(orchid --config ./config create | jq -r '.id')
orchid --config ./config set --id "$ID" --working-dir /path/to/project
orchid --config ./config send --id "$ID" "fix the failing test"
orchid --config ./config await "$ID" --timeout 600
```

### Per-project conversation

```bash
# Create and configure once
ID=$(orchid --config ./config create | jq -r '.id')
orchid --config ./config set --id "$ID" --label my-project --working-dir /path/to/project

# All subsequent sends use the ID
orchid --config ./config send --id "$ID" "add a readme"
orchid --config ./config await "$ID" --timeout 600
orchid --config ./config send --id "$ID" "write tests for the new module"
orchid --config ./config await "$ID" --timeout 600
```

Labels are for human reference only — always use the hex ID in scripts.

## jq recipes

Prefer `orchid get` for orchestration scripts. Direct file reads are useful for
local diagnostics and live streaming, but bypass the CLI's config and resource
validation.

```bash
ID=<session-id>
orchid --config ./config get "$ID" --conversation \\
  | jq '.events | .[-10:]'                                            # last 10 events
orchid --config ./config get "$ID" --conversation \\
  | jq '.events[-1:]'                                                 # most recent event
orchid --config ./config get "$ID" --conversation \\
  | jq '.events | .[-10:][]'                                          # one event per line
orchid --config ./config get "$ID" --last-message \\
  | jq -r '.last_message // empty'                             # latest assistant text
```

For direct local inspection:

```bash
FILE=./config/sessions/<id>/events.jsonl
jq 'select(.type == "message")' "$FILE"                            # messages only
jq 'select(.type == "tool_call") | .calls[] | {name, input}' "$FILE"  # tool calls
jq -s '.' "$FILE"                                                     # full history as array
```
