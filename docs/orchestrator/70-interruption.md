# Interruption and termination

`await` observes. `stop` controls a running session:

```bash
orchid --config ./config stop "$ID"
```

Stopping an agent does not authorize deleting its worktree or branch. Preserve
partial work and inspect it:

```bash
orchid --config ./config get "$ID" --conversation
git -C "$WORKTREE" status --short
git -C "$WORKTREE" diff
git -C "$WORKTREE" log --oneline --decorate -n 10
```

Record the interruption and choose explicitly whether to resume the same session
or assign a new worker. Never reset, clean, remove, or archive work merely
because a worker stopped. A terminal status does not itself mean success.
