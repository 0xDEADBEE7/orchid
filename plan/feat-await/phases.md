# Implementation Phases

## Phase 1 — Command and polling core

- [x] Add `await` command parsing with positional IDs and timeout/interval flags.
- [x] Implement the polling loop with one overall deadline.
- [x] Deduplicate IDs and handle sessions that are already terminal.
- [x] Return all terminal sessions found in a poll.

## Phase 2 — JSON and process contract

- [x] Ensure normal results and errors are JSON-only.
- [x] Implement stable exit codes for completion, timeout, and errors.
- [x] Ensure timeout and interruption leave sessions unchanged.
- [x] Match existing CLI validation and error conventions.

## Phase 3 — Documentation and integration

- [x] Document the launch/collect/await/repeat orchestration workflow.
- [x] Confirm compatibility with scripts that launch multiple asynchronous agents.
- [x] Add examples showing removal of completed IDs before the next await call.
