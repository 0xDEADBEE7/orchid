# Refactor cleanup tracker

Update this document as implementation lands. Keep it factual: mark work done
only after the relevant tests and quality gates have run.

## Delegation log

- **Phase 0 — baseline and hygiene:** complete; implementation commits `b424b9c`, `2781606`, `7b2538e`, `7a1bb93`; formatting commits `2279f43` and `307cf2f`. `make check` and `make metrics` pass. Six metrics red zones remain for later phases.
- **Phase 1 — CLI parser:** active; parser helper extraction `481cc22` reduced `parse` CC from 58 to 41. This recovery run restored the partial visibility/module changes, applied rustfmt, and confirmed `make check` passes; no dispatch extraction was retained because the partial module was not viable. `make metrics` still reports parser `parse` CC 41 and six red files.
- **Phase 1 follow-up — delete validation/parsing:** the delete parser is already owned by `src/cli/delete.rs` from prior parser-helper work before commit `56e3fd9`; this run added characterization coverage for the stable missing-ID error, global-flag preservation, and existing first-ID behavior. No lifecycle/stop/kill or dispatch rewrite was combined. Focused CLI tests pass; the first `make check` was blocked only by rustfmt on the new test spacing, now corrected. `make check` and `make metrics` remain to be rerun.
- **Delegation protocol:** each agent receives the relevant plan links and prior findings, works only in the assigned scope, runs repository-prescribed `make` targets, commits all work even when unsuccessful, and reports commit, tests, failures, and follow-up recommendations. The orchestrator reviews the diff before marking items complete.
- **Phase 2 — execution loop:** stream-response reduction extraction completed in this run. The private helper in `src/loop/stream.rs` owns stream-state ticking and semantic response classification; no provider/session API changed. Focused loop/provider tests pass, `make check`, and `make metrics` pass. `run_loop` CC is 31, `run.rs` is 431 LOC, and six red files remain.
- **Phase 1 — CLI parser:** active; command dispatch extraction from `src/cli/parser.rs` into private `dispatch_command` is complete in the pending commit for this run. The match was moved mechanically; tokenization, global flag merging, command modules, command representations, and error strings were unchanged. Focused existing CLI tests and `make check` pass. `make metrics` reports parser `parse` CC 5, dispatch helper CC 14, parser file 201 LOC, and six red files remain. No focused tests were added.
- **Phase 2 — execution loop:** response handling/continuation decision extraction completed in this run after `53c15f6`. Private `handle_response` now owns tool continuation, terminal message persistence, and empty-response prompting; `RunGuard` ownership, budget checks, lifecycle finalization, event ordering, and public APIs remain in `run_loop`. Focused loop tests, `make check`, and `make metrics` pass. `run_loop` CC is 16 (down from 25), `src/loop/run.rs` is 472 LOC, and six red files remain. The CC<15 target is not yet met; no further extraction was combined.
- **Phase 4 — legacy and documentation cleanup:** not started.
- **Phase 5 — consolidation:** not started.

- **Delegated sessions:** `39446ba8419cda8e456c841728fc8b96` — Phase 0 OpenAI SSE cleanup; completed idle; commit `b424b9c`.
- `d61c564b24b321b574f6b7dbcf661f81` — Phase 1 parser helper extraction; completed idle; commit `481cc22`; `make metrics` passed; `make check` failed only formatting; parser `parse` CC 41.
- **Pending:** no Phase 1 follow-up is pending from this run; the next step is a separate small parser extraction after review.

- [x] Phase 0 — baseline and hygiene
- [ ] Phase 1 — CLI parser
- [ ] Phase 2 — execution loop
- [ ] Phase 3 — provider and session boundaries
- [ ] Phase 4 — legacy and documentation cleanup
- [ ] Phase 5 — consolidation

## Phase 0 — baseline and hygiene

- [x] Run `make metrics` and record 6,258 production cloc / 2,651 test cloc.
- [x] Record five red files and the primary complexity hotspots.
- [x] Run `make check` and record current Clippy failures.
- [x] Fix needless borrows in `src/client/openai/sse.rs`.
- [x] Introduce `SendRequest` for `cmd::send::send`.
- [x] Introduce `ToolContext` for tool execution permissions and scope.
- [x] Fix the release binary name checked by `.scripts/metrics.sh`.
- [x] Re-run `make metrics` after the metrics-script fix (`6,297` production lines; 5 red files remain).
- [x] Re-run `make check`.

## Phase 1 — CLI parser

- [x] Add parser characterization tests where coverage is missing.
- [x] Extract global option parsing.
- [x] Delegate command-specific parsing and validation.
- [x] Preserve obsolete-command rejection behavior.
- [x] Reduce `parse_args` below CC 15.
- [x] Re-run `make check` and `make metrics` (formatting passes; six metrics red files remain).


## Phase 2 — execution loop

- [x] Add characterization tests for current terminal loop outcomes (6 focused tests pass: completion, provider failure, stream failure, tool continuation, budget stop, and cancellation/lifecycle cleanup). `make test`, `make check`, and `make metrics` pass; `run_loop` remains CC 41.
- [x] Define typed loop outcomes/transitions: `provider_turn` now maps provider/stream responses to private `ContinueWithTools`, `Complete`, `Empty`, and `Failed` outcomes; six characterization tests, `make check`, and `make metrics` pass. `run_loop` complexity decreased from CC 41 to CC 38, while `run.rs` is 435 LOC and remains red.
- [x] Extract stream response reduction into `src/loop/stream.rs` via private `reduce_response_stream`; provider stream errors, liveness ticks, complete/empty/tool outcomes, and event ordering remain unchanged. Focused loop (6) and provider (4) tests pass, `make check`, and `make metrics` pass. `run.rs` is 431 LOC; `run_loop` remains CC 31 and six red files remain.
- [x] Extract tool-turn execution into private `execute_tool_turn`; tool permissions/context, event ordering, continuation, tool errors, persisted state, budget checks, and `RunGuard` ownership remain in the existing loop flow. Focused loop tests pass; `run_loop` CC decreased from 38 to 31 and `run.rs` is 447 LOC.
- [x] Centralize terminal lifecycle finalization. `finish_loop` now owns explicit successful terminal finalization/logging, while `terminate_for_budget` centralizes both pre-send and post-send budget terminal persistence, status, error/reason, hook, and end-log behavior through the existing `RunGuard`; no public APIs changed. Focused loop/lifecycle tests, `make check`, and `make metrics` pass. `run_loop` CC decreased from 31 to 25; `run.rs` is 455 LOC; six red files remain.
- [ ] Reduce `run_loop` below CC 15.
- [x] Re-run `make check` and `make metrics` (formatting passes; six metrics red files remain).


## Phase 3 — provider and session boundaries

- [ ] Split Codex auth/token handling from client behavior.
- [ ] Separate provider wire mapping from shared transport.
- [ ] Separate session model, resolution, and filesystem persistence.
- [ ] Preserve provider and on-disk contracts with focused tests.
- [x] Re-run `make check` and `make metrics` (formatting passes; six metrics red files remain).


## Phase 4 — legacy and documentation cleanup

- [ ] Complete [audit.md](audit.md).
- [ ] Remove confirmed dead implementation.
- [ ] Remove/update obsolete tests and references.
- [ ] Reconcile contradictory architecture docs.
- [ ] Archive completed plans where appropriate.

## Phase 5 — consolidation

- [ ] Re-run full metrics.
- [ ] Remove duplication identified by the refactor.
- [ ] Confirm no production red-zone files remain.
- [ ] Compare production LOC against the 5,200 LOC target.
- [ ] Run final `make check`.

## Deferred decisions

- [ ] Decide whether repository/process ports materially improve testing.
- [ ] Decide whether shared types should be split after dependency analysis.
- [ ] Decide whether additional provider clients are supported product behavior.
- [ ] Reassess further LOC reduction against maintainability benefits.
