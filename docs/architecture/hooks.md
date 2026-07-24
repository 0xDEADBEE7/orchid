# Hooks

Hooks are lifecycle integrations configured in the root `config.json`. The
implementation supports `turn-start` and `turn-stop`; hook settings are part of
the effective session configuration snapshot.

## Configuration and CLI

```json
{
  "policy": "default",
  "hooks": {
    "turn-start": ["./scripts/prepare.sh"],
    "turn-stop": ["./scripts/notify.sh"]
  }
}
```

`hooks` is optional. Event values must be arrays of non-empty strings; unknown
event names and malformed values are rejected by `config validate`. The display
contract exposes the resolved object through `config show hooks`:

```bash
orchid --config ./config config validate
orchid --config ./config config show hooks
```

No hook registration or diagnostic invocation command is implemented.

## Runner contract

For every configured event, scripts execute directly and sequentially in list
order. No shell parses the configured path. Relative paths are resolved against
the session working directory, which is also the child process current directory. Each script receives a JSON object on stdin with `event`,
`session_id`, RFC3339 UTC `timestamp`, `status`, and absolute `working_dir`,
`config_dir`, and `session_dir`. Stop payloads may include `error` and `reason`.

The runner sets `ORCHID_EVENT`, `ORCHID_SESSION_ID`, `ORCHID_CONFIG_DIR`,
`ORCHID_SESSION_DIR`, and `ORCHID_WORKING_DIR`. These are convenience variables;
the JSON payload is authoritative.

Scripts run with a per-script timeout and bounded stdout/stderr capture
(currently 2 seconds and 64 KiB). Spawn failures, broken pipes, non-zero exits,
and timeouts are recorded in the session diagnostic log. Failures do not change
the original run result or prevent later hooks, and hook output is never written
to `conversation.jsonl`.

`turn-start` runs after the running transition and logger setup, before the first
provider request. `turn-stop` runs exactly once for terminal outcomes where
execution is possible: success, handled failure, cancellation, budget
termination, and unexpected exits covered by cleanup. A process killed before
cleanup cannot execute a hook; subsequent crash reconciliation makes a
best-effort stop invocation with a failed/crashed context.
