# Testing

## Unit tests

- [x] One running agent remains pending.
- [x] One already-idle agent returns immediately.
- [x] Several agents finish in the same poll and are all returned.
- [x] Failed and cancelled agents count as completed with their statuses.
- [x] Duplicate IDs are handled deterministically.
- [x] Empty IDs produce a JSON usage error.
- [x] Timeout returns an empty completion list and exit code `2`.
- [x] The overall deadline is respected across multiple polls.
- [x] Invalid timeout and interval values are rejected.

## Integration tests

- [x] Launch several sessions, collect their IDs, and await them in batches.
- [x] Process returned IDs, then await only the remaining IDs.
- [x] Verify an agent finishing before `await` starts is still reported.
- [x] Verify polling/API errors produce JSON errors and do not alter sessions.
- [x] Verify Ctrl-C does not cancel observed sessions.

## Lifecycle review

- [x] `stop` and `kill` share `cancelled` semantics under the current CLI contract.
- [x] Crash reconciliation remains `idle` and repairs stale state for future runs.
- [x] Unexpected `RunGuard` exits persist `failed` and are reported by `await`.
- [x] Run failures after `on_run_start` are covered by `RunGuard` in `run_loop`.
