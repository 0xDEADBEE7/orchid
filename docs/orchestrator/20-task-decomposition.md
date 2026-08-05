# Task decomposition

Inspect the repository before delegating. Identify implementation areas, tests,
validation commands, dependencies, file ownership boundaries, risks, and
missing acceptance criteria.

Split work by independently verifiable outcomes, not arbitrary file counts.
Keep each task small enough for one worker to explain, test, and commit.

Use sequence when a task depends on another task's code or findings:

```text
investigate → implement → validate → integrate
```

Use parallelism for independent reviews, documentation, tests, or disjoint
implementation areas. Do not parallelize overlapping edits merely to reduce
elapsed time.
