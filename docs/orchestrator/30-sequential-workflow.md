# Sequential workflow

For dependent work, create one worker at a time and pass verified findings
forward:

```bash
ID=$("$ORCHID_BIN" create --agent worker \
  --label implement-feature --working-dir "$WORKTREE" | jq -r '.id')
"$ORCHID_BIN" send --id "$ID" -- "<task brief>"
```

Then end your turn. When the worker's `<subagent>` report arrives, inspect its
response (`"$ORCHID_BIN" get "$ID" --last-message`), inspect Git, run or review
validation, and follow up before starting dependent work.

Use the same session for a local correction:

```bash
"$ORCHID_BIN" send --id "$ID" -- \
  "The focused test is missing. Add it, run the relevant checks, and create a follow-up commit."
```

Use a new session for independent review, a changed objective, or a stuck
worker needing a fresh perspective.
