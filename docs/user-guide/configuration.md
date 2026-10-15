# Configuration

A configuration root (default `~/.config/orchid`, or `--config DIR`) holds:

```text
config.json              {"log_level": "info", "hooks": {...}}   hooks here run for every agent
agents/<name>.json       one file per agent
prompts/<name>.md        system prompts (.txt also accepted)
connections/<name>.json  LLM endpoints
auth/tokens/<name>.json  Codex OAuth tokens (written by `orchid auth login <name>`)
sessions/<id>/           runtime state, one directory per session
```

## Agents

```json
{
  "prompt": "worker",
  "connection": "codex",
  "tools": ["bash", "fs_read", "fs_edit"],
  "max_tokens": 120000,
  "hooks": {"on-tool-call": [{"script": "hooks/guard.sh", "timeout": 10}]}
}
```

- `prompt` (required): name in `prompts/`.
- `connection`: name in `connections/`, or the built-in `echo`.
- `tools`: tools the model is offered and allowed to run (`bash`, `fs_read`, `fs_edit`).
  File tools are confined to the session's working directory.
- `max_tokens`: input-token cap checked before each request; negative or absent means none.
- `hooks`: run for this agent's sessions after the global hooks in `config.json`.
  See [Hooks](hooks.md).

`orchid create --agent worker` copies the agent into the session (`metadata.json`
field `agent`, prompt text in `prompt.md`); editing an agent later affects new
sessions only. Override the connection per session with
`orchid create --agent worker --connection codex-sol`. Re-copy an agent into an
existing session with `orchid session <id> --agent <name>`. To raise one
session's limit, edit `agent.max_tokens` in its `metadata.json`.

## Connections

```json
{"interface": "openai", "base_url": "http://localhost:1234", "model": "qwen3", "api_key": "env.LMSTUDIO_KEY"}
```

- `interface`: `openai` (alias `local`) for OpenAI-compatible chat completions,
  or `codex` (alias `openai-codex`) for the Codex Responses API.
- `api_key`: `env.NAME` or `file.PATH` (never a literal key); `auth`: an OAuth
  token name for Codex.
- `headers` (values may be `env.NAME`) and `params` (merged into each request body).

## Upgrading from policies

Older versions used `policies/` plus `agents/<name>.json` = `{"policy", "prompt"}`.
The migration script (`plan/simplify/migrate.py`, kept outside the repository) converts agents and session metadata in place
(additive and reversible with `--reverse`); back up the configuration root first
and stop all running sessions.
