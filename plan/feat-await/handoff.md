# Await Feature Handoff

## Current status

The core `orchid await` feature is implemented on branch `feat/await`.
Recent feature commits:

- `091acfe feat: add await command`
- `a78eca7 test: cover await polling behavior`
- `99e43b3 test: verify await CLI process contract`
- `d061721 test: verify await polling deadlines`
- `06ab264 fix: persist failed session status`
- `e941706 fix: persist cancelled session status`

The worktree was clean at handoff.

## Completed

- Parse `orchid await <session-id>...`.
- Support `--timeout` and `--interval` with defaults of 60 and 2 seconds.
- Validate and deduplicate session IDs.
- Poll session state using one overall deadline.
- Treat `idle`, `failed`, and `cancelled` as terminal.
- Return all terminal sessions found in a poll.
- Return JSON results and stable completion/timeout exit codes.
- Preserve observed sessions during timeout.
- Persist unexpected run failures as `failed`.
- Persist `stop` and `kill` operations as `cancelled`.
- Add unit, lifecycle, polling, deadline, and process-level tests.

## Remaining TODO

### 1. Await error and interruption semantics

- Add tests for missing session state and malformed `state.json`.
- Confirm polling errors return the standard JSON error and exit code 1.
- Confirm polling errors do not modify session state or metadata.
- Add Ctrl-C/interruption coverage where practical.
- Confirm interruption does not cancel observed sessions.

Suggested commit:

```text
test: verify await error and interruption behavior
```

### 2. Review lifecycle semantics

- Review whether `stop` and `kill` should share `cancelled` semantics or expose different results.
- Verify crash reconciliation should remain `idle` or become `failed` under the product contract.
- Add integration coverage for a real failed run reaching `await`.
- Check that all run failure paths are covered by `RunGuard`.

### 3. Documentation and help

- Add `await` to user-facing CLI documentation/help if absent.
- Document launch → collect IDs → await → process results → await remaining IDs.
- Include timeout, interval, exit-code, and JSON examples.
- Explain that `await` observes sessions and does not control them.
- Link the documentation from the scripting/user-guide index.

Suggested commits:

```text
docs: add await command help
```

```text
docs: document await orchestration workflow
```

### 4. Complete plan verification

Review `plan/feat-await/phases.md` and `plan/feat-await/testing.md` item by item.
Mark each item complete or document an explicit deferral. In particular, verify:

- Several sessions launched and awaited in batches.
- Completed IDs removed before a subsequent await call.
- A session completed before await starts is reported.
- API/state errors produce JSON errors.
- Ctrl-C leaves sessions unchanged.

### 5. Quality checks

Run the repository-prescribed commands:

```text
make test
make lint
make check
```

`make test` currently passes. `make check` remains blocked by Clippy errors in:

- `src/client/openai/sse.rs` — needless borrows
- `src/cmd/send.rs` — too many arguments
- `src/tools/mod.rs` — too many arguments

These lint issues predate the handoff work or are outside the await scope. Fix them
in separate focused commits unless they block await changes.

## Implementation notes

- `src/cmd/await.rs` contains the polling implementation.
- `src/cli/mod.rs` parses the command and options.
- `src/main.rs` maps timeout to exit code 2 and errors to the normal JSON error path.
- `src/loop/lifecycle.rs` owns run status transitions.
- `src/loop/guard.rs` marks unexpected run exits as failed.
- `src/cmd/stop.rs` marks interrupted runs as cancelled.
- Prefer small commits following the existing imperative style, for example:
  `test: ...`, `fix: ...`, and `docs: ...`.
