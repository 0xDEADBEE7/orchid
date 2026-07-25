# Orchid rewrite: feature contract (draft)

This is the first-pass contract for a smaller implementation. It describes
observable behavior, not the current module layout. Items marked **MUST** are
the compatibility target; **SHOULD** items may be simplified if the simpler
design remains safe and predictable.

## Product model

Orchid is a headless CLI for creating durable LLM conversation sessions,
sending messages, allowing bounded tool use, and observing or controlling the
session from another process.

## CLI contract

**MUST** support:

- `help`, including command-specific help and stable non-zero errors.
- `create`, `send`, `list`, `get`, `set`, `delete`.
- `await` for observing one or more sessions with timeout and polling interval.
- `stop`/`kill` for terminating a running session.
- `config validate`, `config list`, and `config show`.
- `auth list`, `auth validate`, and provider login where supported.
- `--config`, session ID, label, working directory, policy, prompt, and
  `--await` behavior.
- Machine-readable JSON output and structured JSON errors.

**SHOULD** preserve the current positional/flag forms and aliases. The
parser itself may be replaced with a conventional argument parser.

## Sessions and persistence

**MUST** provide stable IDs, durable metadata, durable state, and an append-only
conversation/event transcript. A process restart must not lose completed
events.

**MUST** represent at least idle, running, failed, and cancelled states;
record the running process ID; detect/reconcile crashed runs; and make delete
reversible through archival rather than immediate destruction.

**SHOULD** retain the current on-disk layout only if compatibility is required.
A new version may use one canonical session document plus an event log.

## Run loop

**MUST** support multi-turn provider interaction, streamed assistant text,
reasoning where available, tool calls/results, transcript updates, bounded
steps, cancellation, and token warning/hard limits.

The loop must make state transitions explicit and must leave a recoverable
record when a run fails at any point.

## Providers

**MUST** retain the shared provider behavior: normal responses, streaming,
tool calls, usage reporting, retries for transient failures, and auth errors.

The current compatibility target includes OpenAI-compatible, Anthropic, and
Codex-style connections. Provider-specific wire formats and SSE decoding may
be isolated behind small adapters rather than exposed to the run loop.

## Configuration

**MUST** support resource-oriented configuration for root settings,
connections, policies, prompts, and authentication profiles. Unknown fields
must fail validation, policy selection must be deterministic, and resolved
configuration must include a stable policy hash.

**MUST** support policy permissions for tools and paths, environment settings,
connection selection, prompts, and limits for steps/tokens.

## Tools and safety

**MUST** retain `fs_read`, `fs_edit`, and `bash` tools, with policy-based tool
authorization and path restrictions. File writes must be atomic where
possible, and shell execution must be bounded by the configured scope.

Path expansion/normalization, symlink/escape behavior, command parsing, and
environment inheritance require explicit security tests in the rewrite.

## Hooks and observability

**SHOULD** retain lifecycle hooks for run start, successful end, failure, and
crash reconciliation, including bounded output and timeout/termination
behavior. Logging and diagnostics must not corrupt JSON command output.

## Compatibility decisions still required

1. Is exact on-disk session compatibility required, or only migration support?
2. Are all three current provider families required in the first rewrite?
3. Is `bash` intended to remain a first-class tool, or can it be an optional
   adapter?
4. Must hooks remain arbitrary executables, or can they become a simpler event
   mechanism?
5. Is the current CLI output schema a public API?

## Initial sizing implication

With the MUST set above and all three provider families included, target
approximately 4–5k production LOC. A 3–4k target becomes credible if the
session format, CLI parser, hook model, and provider codecs are intentionally
simplified while preserving behavior at the command and safety boundaries.
