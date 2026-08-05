# Parallel workflow

Use one Git worktree and one Orchid session per active worker. Put worktrees
under:

```text
~/.worktrees/<repo-name>/<worktree-name>/
```

Create each branch and worktree from the intended base:

```bash
mkdir -p "$HOME/.worktrees/$REPO_NAME"
git -C "$REPO" worktree add \
  -b "orchid/$TASK" "$HOME/.worktrees/$REPO_NAME/$TASK" main
```

Give each worker its own `--working-dir`, launch independent sessions, then
await their IDs together. Parallelize only when workers cannot conflict through
shared files, generated outputs, services, or branches.

Validate every branch before merging. Choose a merge order, merge one branch,
run checks, then continue. Resolve conflicts deliberately and rerun validation.
Never place two active workers in one worktree or ask workers to merge each
other's branches.
