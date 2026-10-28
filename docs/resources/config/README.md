# Resource template

This directory is a clean baseline template for `./config/`, based on the declarative resources in `.test-config/`.

Copy it to create a local configuration tree:

```bash
cp -R docs/resources/config ./config
orchid --config ./config agent
```

## Contents

- `config.json` — root settings.
- `agents/default.json` — the default agent: prompt, connection, tools, token cap, optional hooks.
- `connections/codex.json` — Codex connection; run `orchid auth login codex` once.
- `prompts/default.md` — default system prompt.

Add credentials separately with environment-backed references or `orchid auth login`. Runtime session data is created under `./config/sessions/` and should not be copied into this template.

See the [configuration guide](../../user-guide/configuration.md) for schemas and [hooks](../../user-guide/hooks.md) for the notify-parent hook.
