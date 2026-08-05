# UX Patterns

These patterns reflect the current agent-based session workflow.

## Select an agent when creating the session

```bash
ID=$(orchid --config ./config create --agent default \
  --label my-project | jq -r '.id')
```

An agent resolves the policy and prompt together. The selected configuration is
snapshotted into the session, so later configuration changes do not silently
change an existing session. If omitted, `create` uses the `default` agent.

Capture and reuse the session ID. Labels are annotations, not addressing
handles.

## Set session metadata before sending

```bash
orchid --config ./config set "$ID" \
  --working-dir /path/to/project \
  --label my-project
```

`working-dir` scopes the model's `bash`, `fs_read`, and `fs_edit` tools. Set it
before the first turn when the agent needs to work on files.

## Use the explicit create → send → await → get cycle

```bash
orchid --config ./config send --id "$ID" "Run the tests and summarise failures."
orchid --config ./config await "$ID" --timeout 600
orchid --config ./config get "$ID" --last-message
```

`send` starts a background run and returns immediately. `await` only observes
the run; it does not send another message or modify the session. `get` reads the
result without modifying the session.

`send --await` is not supported. Use `await` after `send` instead.

## Continue an idle session

```bash
orchid --config ./config send --id "$ID" "Now fix the highest-priority failure."
orchid --config ./config await "$ID" --timeout 600
```

Do not send while the session is `running`; wait for `await` to report a
terminal status first. Normal completion is `idle`; `failed` and `cancelled`
require investigation before continuing.

## Inspect the complete conversation

```bash
orchid --config ./config get "$ID" --conversation
```

The durable append-only transcript is stored as `events.jsonl` under the
session directory. For live low-level observation:

```bash
tail -f ./config/sessions/$ID/events.jsonl | jq .
```

Prefer `get` for scripts and consumers because it reads through the CLI rather
than depending on the on-disk layout.

## Run multiple independent tasks

Create one session per independent task and choose an agent for each:

```bash
IDS=()
for task in "run tests" "review the diff"; do
  ID=$(orchid --config ./config create --agent default | jq -r '.id')
  orchid --config ./config send --id "$ID" "$task"
  IDS+=("$ID")
done
orchid --config ./config await "${IDS[@]}" --timeout 600
```

Use the agent resource for reusable policy and prompt choices. Use `set` for
session-specific metadata such as labels and working directories.
