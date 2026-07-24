# Hooks implementation tracker

Update this document as work lands. Keep the checklist as the high-level source
of progress; implementation details belong in the linked phase documents.

## Delegation log

- **Phase 1 — contract and configuration:** completed; typed hook contract, root integration, validation, effective configuration snapshot, and focused tests committed.

## Delivery checklist

- [x] **Phase 1 — contract and configuration**: define hook types, schema, and effective configuration resolution. See [phases.md](phases.md) and [schema.md](schema.md).
- [x] **Phase 2 — hook runner**: execute scripts with JSON stdin, working-directory support, limits, and diagnostics. See [execution.md](execution.md).
- [ ] **Phase 3 — lifecycle integration**: wire `turn-start` and exactly-once `turn-stop` into all terminal paths. See [execution.md](execution.md).
- [ ] **Phase 4 — CLI and documentation**: validate, display, and document hook configuration and behavior. See [cli.md](cli.md).
- [ ] **Phase 5 — testing and hardening**: complete automated coverage, security review, and regression verification. See [testing.md](testing.md) and [phases.md](phases.md).

## Phase 1 — contract and configuration

- [x] Define typed events for `turn-start` and `turn-stop`.
- [x] Add optional hooks to the root configuration schema.
- [x] Validate event names, script lists, and non-empty script entries.
- [x] Decide and implement script path resolution relative to the session working directory.
- [x] Add hooks to effective session configuration.
- [x] Ensure hook configuration is stable for the lifetime of a run.
- [x] Define the shared JSON payload and terminal status/error fields.
- [x] Add configuration unit tests.

## Phase 2 — hook runner

- [x] Add a dedicated `src/hooks/` module.
- [x] Invoke executables directly without shell interpretation.
- [x] Set the child process current directory to the session working directory.
- [x] Serialize the event payload to stdin.
- [x] Set documented `ORCHID_*` environment variables.
- [x] Execute scripts sequentially in registration order.
- [x] Add per-script timeout handling.
- [x] Bound captured stdout and stderr.
- [x] Return per-script results without stopping the remaining hook sequence.
- [x] Log spawn failures, non-zero exits, timeouts, and output details safely.
- [x] Add runner unit and process-level tests.

## Phase 3 — lifecycle integration

- [ ] Invoke `turn-start` after configuration resolution, crash reconciliation, state transition, and logger setup.
- [ ] Ensure `turn-start` runs before the first provider request.
- [ ] Centralize terminal cleanup so `turn-stop` has one owner.
- [ ] Track whether `turn-stop` has already fired.
- [ ] Invoke `turn-stop` on successful completion.
- [ ] Invoke `turn-stop` on provider and tool-loop failures.
- [ ] Invoke `turn-stop` on token-budget termination.
- [ ] Invoke `turn-stop` on cancellation.
- [ ] Extend `RunGuard` for best-effort unexpected-exit handling.
- [ ] Prevent hook failures from replacing the original run result.
- [ ] Add exactly-once lifecycle tests.

## Phase 4 — CLI and documentation

- [ ] Include hook validation in `orchid config validate`.
- [ ] Add hook output to `orchid config show hooks`, if supported by the existing display contract.
- [ ] Document the configuration format.
- [ ] Document stdin payload fields and environment variables.
- [ ] Document working-directory and direct-execution behavior.
- [ ] Document ordering, timeout, failure, and crash semantics.
- [ ] Add a minimal example hook script.

## Phase 5 — testing and hardening

- [ ] Test missing hooks and empty hook lists.
- [ ] Test malformed configuration and unknown events.
- [ ] Test payload validity and required fields.
- [ ] Test working-directory behavior and relative paths.
- [ ] Test ordering and continuation after a failed hook.
- [ ] Test missing executables, non-zero exits, timeouts, and bounded output.
- [ ] Test that shell metacharacters are not interpreted.
- [ ] Test every terminal lifecycle path and duplicate prevention.
- [ ] Test best-effort crash reconciliation behavior.
- [ ] Run the repository-prescribed `make check` target.
- [ ] Review the final implementation against [overview.md](overview.md).

## Deferred work

- [ ] Decide whether session-level hook overrides are needed.
- [ ] Decide whether a `hook test` diagnostic command is needed.
- [ ] Evaluate additional events such as `session-created`, `tool-start`, `tool-stop`, `model-request`, and `model-response`.
- [ ] Define a separate policy for hooks that are allowed to block or fail a run.
