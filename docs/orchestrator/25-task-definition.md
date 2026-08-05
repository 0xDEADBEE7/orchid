# Task definition

Define every worker task with the same four sections. Keep the task specific,
verifiable, and small enough for one worker to complete and commit.

## Task Definition

State the single outcome the worker owns. Include:

- the action required;
- the relevant component or behavior;
- the assigned repository and worktree;
- the branch name;
- explicit boundaries and non-goals;
- the expected commit behavior.

Do not combine investigation, unrelated implementation, and broad cleanup in
one task unless they are inseparable.

## Success Criteria

State observable completion conditions. Include:

- required behavior or artifacts;
- tests, checks, or commands that must pass;
- files or interfaces that must remain compatible;
- required documentation or migration updates;
- the expected final Git state.

Criteria should let a validator decide pass or fail without guessing. Require
the worker to report the exact commands it ran and their results.

## Additional Context

Provide information that helps the worker work consistently without expanding
the task. Include repository conventions, compatibility constraints, known
risks, relevant prior findings, and handoff expectations.

Tell the worker to read `.agents/README.md` first when it exists. It is the
preferred source for repository-specific instructions, tool conventions,
validation commands, and commit formalities.

When history matters, tell the worker how to inspect it:

```bash
git log --oneline --decorate -n 20
git log -- path/to/relevant/file
git show <commit>
```

Require focused commits, no unrelated cleanup, no rewritten existing commits,
and a report containing changes, validation, commit IDs, worktree status, and
remaining issues.

## Sources

List a small set of highly relevant files, directories, commits, or external
references. Each source should have a purpose. Prefer authoritative sources
such as the implementation, tests, schema, interface, or existing design
contract. Do not list the whole repository.

```text
Sources:
- src/parser.rs — current parser implementation
- test/parser.rs — behavioral examples and regression coverage
- docs/parser.md — public contract
- <commit> — prior decision that constrains compatibility
```

The worker should inspect these sources before editing and treat conflicts in
this order: explicit task constraints, repository instructions, authoritative
implementation/tests/contracts, then explanatory background.

## Task template

Use this structure when delegating:

```markdown
## Task Definition
[One concrete outcome, worktree, branch, boundaries, and non-goals]

## Success Criteria
[Observable behavior, validation commands, and final Git state]

## Additional Context
[Repository instructions, conventions, prior findings, and handoff rules]

## Sources
- [path or commit] — [why it matters]
- [path or commit] — [why it matters]
```
