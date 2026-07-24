# Refactor cleanup tracker

Update this document as implementation lands. Keep it factual: mark work done
only after the relevant tests and quality gates have run.

## Delegation log

- **Phase 0 — baseline and hygiene:** complete; implementation commits `b424b9c`, `2781606`, `7b2538e`, `7a1bb93`; formatting commits `2279f43` and `307cf2f`. `make check` and `make metrics` pass. Six metrics red zones remain for later phases.
- **Phase 1 — CLI parser:** active; parser helper extraction `481cc22` reduced `parse` CC from 58 to 41. This recovery run restored the partial visibility/module changes, applied rustfmt, and confirmed `make check` passes; no dispatch extraction was retained because the partial module was not viable. `make metrics` still reports parser `parse` CC 41 and six red files.
- **Phase 1 follow-up — send validation/parsing extraction:** complete in this run; moved send message validation, option extraction, command construction, and unknown-flag rejection from `src/cli/parser.rs` to `src/cli/send.rs`. Preserved exact errors, message handling, flags, command representation, and global flag merging. Focused CLI tests, `make check`, and `make metrics` pass; parser is 199 LOC and `parse` is CC 30; six red files remain.
- **Delegation protocol:** each agent receives the relevant plan links and prior findings, works only in the assigned scope, runs repository-prescribed `make` targets, commits all work even when unsuccessful, and reports commit, tests, failures, and follow-up recommendations. The orchestrator reviews the diff before marking items complete.
- **Phase 1 — CLI parser:** active; remaining work is reducing parser dispatch complexity below the phase exit criterion.
- **Phase 2 — execution loop:** not started.
- **Phase 3 — provider and session boundaries:** not started.
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

- [ ] Add characterization tests for all terminal loop outcomes.
- [ ] Define typed loop outcomes/transitions.
- [ ] Extract stream response reduction.
- [ ] Extract tool-turn execution.
- [ ] Centralize terminal lifecycle finalization.
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
