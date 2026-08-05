# Sequential workflow

For dependent work, create one worker at a time and pass verified findings
forward:

```bash
ID=$(orchid --config ./config create --agent worker \
  --label implement-feature --working-dir "$WORKTREE" | jq -r '.id')
orchid --config ./config send --id "$ID" "<task brief>"
orchid --config ./config await "$ID" --timeout 900
orchid --config ./config get "$ID" --last-message
```

After each worker, inspect its response, inspect Git, run or review validation,
and follow up before starting dependent work.

Use the same session for a local correction:

```bash
orchid --config ./config send --id "$ID" \
  "The focused test is missing. Add it, run the relevant checks, and create a follow-up commit."
```

Use a new session for independent review, a changed objective, or a stuck
worker needing a fresh perspective.
