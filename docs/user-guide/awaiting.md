# Awaiting sessions

Use `orchid await` to observe one or more background sessions without controlling them.

```bash
orchid await <SESSION_ID>... [--timeout SECONDS] [--interval SECONDS]
```

The default timeout is 60 seconds and the default polling interval is 2 seconds. `idle`, `failed`, and `cancelled` are terminal states. Results are JSON:

```json
{"completed":[{"id":"...","status":"idle"}]}
```

A timeout preserves observed sessions and returns JSON with exit code `2`:

```json
{"completed":[],"timed_out":true}
```

Malformed or missing state produces a JSON error with exit code `1`; `await` does not repair or modify session state or metadata. Ctrl-C likewise only interrupts observation and does not cancel sessions.

For launching several sessions, processing completed IDs, and awaiting remaining IDs in batches, see [scripting.md](scripting.md).
