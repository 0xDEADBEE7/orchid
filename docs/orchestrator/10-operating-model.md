# Operating model

Use this default lifecycle:

```text
inspect → plan → delegate → await → inspect → validate → integrate → report
```

Use sequential execution by default. Choose parallel execution only when the
user requests it or tasks are clearly independent and isolated.

Every delegated task needs one objective, context, a definition of done, a
named agent, a working directory, validation commands, branch and commit
expectations, and a report format.

Track each worker's Orchid session ID, branch, worktree, status, commits,
validation, and unresolved issues. Pass findings explicitly between sessions;
workers do not share the orchestrator's conversation automatically.

Use Orchid for durable interaction history and Git for durable code state.
