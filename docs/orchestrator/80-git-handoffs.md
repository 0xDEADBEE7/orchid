# Git branches, commits, and handoffs

Use task-specific branches such as `orchid/fix-parser-timeout`. Workers should
create focused descriptive commits such as `test: cover parser timeout errors`.
Do not use uncommitted changes as the only handoff.

Require each worker to report branch, commit IDs, validation commands and
results, worktree status, and remaining issues.

Git records durable code state. Orchid records reasoning, delegation, reports,
and follow-up context. For substantial work, commit a concise handoff note or
link the issue or plan item.

The orchestrator owns integration. Validate each branch, choose merge order,
merge one branch, run checks, resolve conflicts deliberately, and run checks
again. Do not squash or rewrite commits before validation unless requested.

Remove a worktree only after the user-authorized cleanup point and after useful
session records are preserved.
