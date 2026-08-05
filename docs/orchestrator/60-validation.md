# Validation and follow-up

Before accepting a worker, inspect:

```bash
git -C "$WORKTREE" status --short
git -C "$WORKTREE" diff --check
git -C "$WORKTREE" diff --stat main...HEAD
git -C "$WORKTREE" log --oneline --decorate -n 10
```

Confirm intended files only, acceptance criteria, focused and broader checks,
absence of secrets or generated artifacts, expected commits, and a clean or
understood worktree.

Use a precise follow-up that names the observed gap and required evidence. Do
not say only “try again.” Reinspect the new diff afterward.

Classify validation failures as implementation, test, environment, integration,
or requirement ambiguity. Fix the smallest concrete cause or ask for clarity.
