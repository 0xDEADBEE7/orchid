# Interruption and termination

Worker reports and `get` observe. `stop` controls a running session (it ends the
worker and its tool commands, and marks the session `cancelled`):

```bash
"$ORCHID_BIN" stop "$ID"
```

Stopping an agent does not authorize deleting its worktree or branch. Preserve
partial work and inspect it:

```bash
"$ORCHID_BIN" get "$ID" --conversation
git -C "$WORKTREE" status --short
git -C "$WORKTREE" diff
git -C "$WORKTREE" log --oneline --decorate -n 10
```

Record the interruption and choose explicitly whether to resume the same session
or assign a new worker. Never reset, clean, remove, or archive work merely
because a worker stopped. A terminal status does not itself mean success.
