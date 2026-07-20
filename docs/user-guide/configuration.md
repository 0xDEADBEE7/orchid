# Configuration

The new configuration is a directory containing independent resources. Select
it with `--config <directory>`:

```bash
orchid --config ./config validate
```

If omitted, Orchid uses its documented default configuration directory. The
selected directory is self-contained; Orchid does not merge it with another
configuration directory.

## Layout

```text
config/
  config.json
  connections/
    anthropic.json
  policies/
    default.json
  prompts/
    engineering.md
  sessions/
```

## Root config

`config.json` selects the default policy:

```json
{
  "policy": "default"
}
```

## Connection

`connections/anthropic.json`:

```json
{
  "interface": "anthropic",
  "base_url": "https://api.anthropic.com",
  "api_key": "env.ANTHROPIC_API_KEY",
  "model": "claude-sonnet-4-6",
  "params": {
    "max_tokens": 8192
  }
}
```

API keys must use `env.<VAR>` references. Resolved values are read at runtime
and are never written to disk or included in diagnostic output.

## Policy

`policies/default.json`:

```json
{
  "connections": ["anthropic"],
  "prompt": "engineering",
  "permissions": {
    "tools": ["bash", "fs_read", "fs_edit"],
    "paths": ["/tmp/**"]
  },
  "limits": {
    "token_warn_threshold": 80000,
    "token_hard_limit": 120000
  }
}
```

Connections are tried in declared order. Policies own routing, permissions,
prompts, and execution limits.

## Prompts

Prompts are UTF-8 Markdown files under `prompts/`. A policy references a prompt
by filename stem. Missing resources cause validation to fail before a run.

## Local development

Use a repository-local directory to develop without affecting an existing
installation:

```bash
mkdir -p config/{connections,policies,prompts,sessions}
orchid --config ./config validate
```

Do not commit secrets. Use environment references in Connection files.
