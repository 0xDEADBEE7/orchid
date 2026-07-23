# `orchid get`

Read selected session resources without modifying the session.

```bash
orchid --config ./config get <SESSION_ID> --state --metadata
orchid get <SESSION_ID> --conversation --config ./config
```

Selectors are `--conversation`, `--last-message`, `--metadata`, and `--state`.
At least one selector is required; fields are returned in deterministic order.
The session ID must be a 32-character hexadecimal ID. Missing or malformed
resources produce a JSON error and exit code 1.

`--logs` remains deferred from this focused release and is rejected as an
unsupported selector. Reads stay within the selected `--config` directory and
are safe for running sessions.
