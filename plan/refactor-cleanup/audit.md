# Cleanup audit

This document records evidence before removal. Mark an item confirmed only
after code references, tests, docs, and runtime behavior have been checked.

## Legacy references

- [x] Trace profiles/personas and old configuration terminology in production.
- [x] Trace old conversation paths and migration adapters in production.
- [ ] Classify compatibility-rejection tests as current contract or obsolete.
- [ ] Classify historical plans as active, completed, or archival.
- [x] Search for `TODO`, `FIXME`, `dead_code`, and broad lint allowances.

## Duplication candidates

- [ ] Compare provider HTTP/retry/error handling before consolidating.
- [ ] Compare Anthropic/OpenAI/Codex stream accumulation and event mapping.
- [ ] Compare session JSON loading, updating, and atomic write helpers.
- [ ] Compare command config/session resolution and output formatting.
- [ ] Compare tool permission and path-scope validation.

## Documentation drift

- [x] Update `docs/architecture/providers.md` from obsolete Go-style paths/types to
  the Rust `Provider` trait and current client modules.
- [x] Reconcile `docs/architecture/execution.md` and `storage.md` with the
  actual `conversation.jsonl`, `state.json`, `metadata.json`, and `orchid.log`
  behavior, including atomic JSON persistence and PID-based crash recovery.
- [x] Reconcile the provider list in `docs/README.md` with the supported
  Anthropic, OpenAI-compatible, and Codex OAuth clients.
- [x] Correct current configuration claims in `docs/architecture/NEW_CONFIG.md`
  where the documented policy schema or provider support was broader than code.
- [ ] Link every current architecture document to this plan while refactoring;
  deferred because it would add navigation rather than correct a factual
  contradiction.

## Completion notes

The requested documentation reconciliation is complete. Reviewed the current
Rust provider, configuration, session, lifecycle, logging, and SSE modules
against the architecture and user-guide docs. Confirmed current auth profiles,
ordered provider fallback, JSONL session events, separate state/metadata files,
atomic state/metadata writes, PID crash reconciliation, and best-effort
`orchid.log` diagnostics. No Rust code, tests, or public contracts were changed.

The production search found current auth-profile terminology and compatibility
helpers, plus narrowly scoped `dead_code` allowances in the Anthropic SSE
implementation. These were not removed because reachability and compatibility
contracts were not established by documentation review alone.

## Deferred audit work

The broader legacy and duplication audit remains open. Production references,
compatibility tests, historical plan classification, lint markers, and
possible code removal were not changed without a confirmed obsolete contract.
Plan archiving is also deferred as requested.