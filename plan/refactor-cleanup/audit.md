# Cleanup audit

This document records evidence before removal. Mark an item confirmed only
after code references, tests, docs, and runtime behavior have been checked.

## Legacy references

- [x] Trace profiles/personas and old configuration terminology in production.
- [x] Trace old conversation paths and migration adapters in production.
- [x] Classify compatibility-rejection tests as current contract or obsolete. The CLI tests in `tests/test_cli.rs` are contractual: `phases.md` explicitly requires rejection of obsolete commands and exact errors, and the tests protect those stable messages. `tests/test_cmd_config.rs` is also contractual: `fs_edit` intentionally rejects the removed single-edit schema. No compatibility-rejection test is obsolete.
- [x] Classify historical plans as active, completed, or archival. See the classification below; no plan is archived because candidates are useful history and the repository has no archive convention.
- [x] Search for `TODO`, `FIXME`, `dead_code`, and broad lint allowances.

## Duplication candidates

- [x] Compare provider HTTP/retry/error handling before consolidating. Anthropic and OpenAI use the shared `BaseClient::post_with_retry`; Codex uses the same client boundary while retaining its distinct OAuth/request behavior. No duplicate retry implementation was found.
- [x] Compare Anthropic/OpenAI/Codex stream accumulation and event mapping. The shared SSE parser owns framing and common accumulation; Anthropic and OpenAI mappers own incompatible wire/event semantics, while Codex uses its own response mapping. Consolidation would change provider behavior, so no code was removed.
- [x] Compare session JSON loading, updating, and atomic write helpers. Persistence owns update/atomic-write behavior; resolution retains the public read/ID-validation boundary. Similar reads have different error/API responsibilities and are not confirmed dead duplication.
- [x] Compare command config/session resolution and output formatting. `create`, `send`, and `__run` share the config resolver; command-specific orchestration and output paths remain distinct. No unused forwarding layer or obsolete formatter was confirmed.
- [x] Compare tool permission and path-scope validation. `src/tools/scope.rs` is the shared policy/path boundary used by bash, fs-read, and fs-edit; their separate checks reflect different tool inputs. No duplicate implementation was removed.

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

## Historical plan classification

- **Completed, retain as useful history:** `plan/config-refactor/` (the current resource/config/session model is implemented), `plan/feat-await/` (the await command and tests exist), and the completed portions of `plan/feat-hook/` (hooks are implemented and wired; its explicitly deferred decisions remain).
- **Active or partially implemented, retain in place:** `plan/emacs-client/`, `plan/get-feature/`, and `plan/scope-enforcement/` contain unchecked work or future integration decisions.
- **Archive candidate, but not archived:** `plan/server-actions/` describes a rejected command surface and is contradicted by the current CLI, but it is useful migration history and has no clear archive convention in this repo. `plan/hooks/` is a short proposal superseded by `plan/feat-hook/`, but it is likewise retained because archiving would add no safety or clarity.
- **Current source of truth:** `docs/` and the Rust modules; historical plans are not treated as current contracts.

## Completion notes

The remaining audit found no confirmed dead production implementation, obsolete behavior test, or stale current-reference that can be removed safely without changing a CLI, storage, provider, or tool contract. Provider transport and common SSE framing are already shared; provider mapping remains intentionally separate. Session, command, and tool similarities were reviewed and retained where their ownership or public error behavior differs. The scoped `dead_code` allowances in Anthropic SSE remain because their trait callbacks are reachable through the shared SSE parser.

## Deferred audit work

No implementation or test removal is justified by this audit. Plan archiving is deferred because the candidates are useful history and the repository has no archive convention. Linking every architecture document to this plan remains a navigation-only improvement and is also deferred.