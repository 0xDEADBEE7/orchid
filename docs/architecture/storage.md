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
      conversation.jsonl
      metadata.json
      state.json
      orchid.log
```

Select the directory with `--config <directory>`. If omitted, Orchid uses the
documented default configuration directory. The selected directory is
self-contained; Orchid does not read or write old global config, prompt, or
conversation paths.

- `config.json` — default policy selection. See [config.md](config.md).
- `connections/` — callable inference endpoints.
- `policies/` — routing, prompts, permissions, and limits.
- `prompts/` — reusable Markdown documents.
- `sessions/` — durable work and execution state.

## Session files

- `conversation.jsonl` — append-only chronological events.
- `metadata.json` — identity and resource references.
- `state.json` — mutable status and execution state.
- `orchid.log` — diagnostics separate from conversation history.

A local configuration directory allows development without interrupting another
installation:

```bash
orchid --config ./config validate
```
