# Config

The new configuration is a self-contained directory selected with
`--config <directory>`. See [storage.md](storage.md) for the layout.

## Root schema

`config.json` contains the default policy name:

```json
{
  "policy": "default"
}
```

## Connection schema

Each `connections/<name>.json` defines one inference endpoint:

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

Connection fields contain endpoint and provider details only. API keys must use
`env.<VAR>` references and are resolved at runtime.

## Policy schema

Each `policies/<name>.json` defines execution behavior:

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

The ordered `connections` list controls routing. Policies own prompts,
permissions, and limits; sessions may only narrow those permissions.

## Prompts

Prompts are Markdown files under `prompts/`. A policy references a prompt by
name. Missing resources are validation errors.

## Configuration directory

Use a local directory to isolate the new setup from any existing tooling:

```bash
orchid --config ./config validate
orchid --config ./config list policies
```

The selected directory is not merged with or read alongside another directory.
