# Hooks

Hooks are configured globally in `config.json` and run at turn boundaries. They
are optional; an absent or empty `hooks` object disables them.

```json
{
  "policy": "default",
  "hooks": {
    "turn-start": ["./scripts/prepare.sh"],
    "turn-stop": ["./scripts/notify.sh"]
  }
}
```

Validate the complete configuration, including hook event names and entries:

```bash
orchid --config ./config config validate
orchid --config ./config config show hooks
```

`config show hooks` returns the resolved hook object. It returns empty arrays for
events that are not configured. There is no `orchid hook` or `hook test`
command.

## Execution contract

- Entries are executable paths, not shell command strings. Orchid invokes each
  path directly, without shell interpretation.
- Relative paths resolve from the session working directory. Each child also
  uses that directory as its current working directory.
- Hooks run sequentially in the order listed.
- Each hook receives one JSON object on stdin. The payload includes `event`,
  `session_id`, RFC3339 UTC `timestamp`, `status`, absolute `working_dir`,
  `config_dir`, and `session_dir`. Stop payloads may also include `error` and
  `reason`.
- Convenience environment variables mirror the context:
  `ORCHID_EVENT`, `ORCHID_SESSION_ID`, `ORCHID_CONFIG_DIR`,
  `ORCHID_SESSION_DIR`, and `ORCHID_WORKING_DIR`. The JSON payload is the
  source of truth.
- Output is captured with bounded stdout/stderr and each script has a
  per-script timeout (currently 2 seconds and 64 KiB respectively).

Hook failures (including spawn errors, non-zero exits, broken pipes, and
timeouts) are written to diagnostics, do not fail the agent run, and do not
prevent later hooks from running. Hook output is not added to conversation
history.

`turn-start` runs after the run is marked running and before the first provider
request. `turn-stop` runs once for terminal outcomes when execution is possible,
including success, handled failures, cancellation, and budget termination. A
process killed without cleanup cannot run a hook; later reconciliation makes a
best-effort crashed stop notification where context is available.

See [architecture/hooks.md](../architecture/hooks.md) for the implementation
contract.
