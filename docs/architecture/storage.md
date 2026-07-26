# Storage Layout

The new implementation stores resources and sessions below one selected
configuration directory:

```text
./config/
  config.json
  connections/
    anthropic.json
  policies/
    default.json
  prompts/
    engineering.md
  sessions/
    <id>/
      events.jsonl
      metadata.json
      logs.jsonl
```

Select the directory with `--config <directory>`. If omitted, Orchid uses the
documented default configuration directory. The selected directory is
self-contained; Orchid does not read or write old global config, prompt, or
conversation paths.

- `config.json` — default policy selection. See [config.md](config.md).
- `connections/` — callable inference endpoints.
- `policies/` — ordered connection candidates, prompts, permissions, and limits.
- `prompts/` — reusable Markdown documents.
- `sessions/` — durable work and execution state.

## Session files

The persisted session consists of three JSON files plus a diagnostic log:

- `conversation.jsonl` is the append-only transcript of typed message,
  tool-call, tool-result, and reasoning events. It is read to reconstruct
  provider history.
- `metadata.json` contains session identity, configuration references, and
  mutable execution state such as status, PID, timestamps, token estimates,
  and termination information. The latest message is derived from events.
- `logs.jsonl` is best-effort newline-delimited JSON diagnostics; it is not the
  conversation transcript or a state store.

Metadata writes use temporary files followed by rename. Events are appended
directly, and diagnostic logging is best-effort.

To validate the selected resource tree:

```bash
orchid --config ./config validate
```
