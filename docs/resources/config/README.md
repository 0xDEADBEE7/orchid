# Resource template

This directory is a clean baseline templat for `./config/`, based on the declarative resources in `.test-config/`.

Copy it to create a local configuration tree:

```bash
cp -R resources ./config
orchid --config ./config config validate
```

## Contents

- `config.json` — root settings.
- `agents/default.json` — default policy and prompt selection.
- `connections/codex.json` — optional Codex-compatible connection.
- `policies/default.json` — baseline permissions and token limit.
- `prompts/default.md` — default system prompt.

Add credentials separately with environment-backed references or `orchid auth login`. Runtime session data is created under `./config/sessions/` and should not be copied into this template.

See [configuration](docs/user-guide/configuration.md) and the [configuration architecture](docs/architecture/NEW_CONFIG.md) for schemas.
