# CLI reference

Every command prints one JSON object on stdout. Errors print
`{"error":"command_failed","message":"…"}` on stderr and exit 1. `--config DIR`
(or `ORCHID_CONFIG`) selects the configuration root, default `~/.config/orchid`.

| Command | Does | Prints |
|---|---|---|
| `create [--agent A] [--connection C] [--label L] [--working-dir D] [--parent ID]` | New session from agent `A` (default `default`) | `{id, status, agent}` |
| `send --id ID [--no-run] [--] MESSAGE` | Stores the message and starts a worker; queues it if a worker is running | `{id, status: "running", pid}`, `{…, status: "queued", event_id}`, or with `--no-run` `{id, status, no_run}` |
| `await ID… [--timeout S]` | Waits (default 60s) until no session is running | `{sessions: [{id, status, message, reason}]}`; exit 0 all idle, 1 any failed/cancelled/terminated, 2 timeout |
| `get ID [--last-message]` | Reads a session (never waits or changes it) | the session, or `{id, last_message}` |
| `list` | Lists sessions | `{sessions: [{id, label}]}` |
| `set ID [--label L] [--working-dir D]` | Edits metadata | `{id, updated}` |
| `session ID --agent A` | Re-copies an agent (and its prompt) into a session | `{id, agent, updated}` |
| `stop ID` (alias `kill`) | Ends the worker and its tool commands; marks the session `cancelled` | `{id, status}` |
| `delete ID` | Moves the session to `archive/` | `{id, archived}` |
| `agent` | Lists agents and whether they resolve | `{agents: [{name, prompt, connection, valid, error?}]}` |
| `auth list\|add\|test\|login …` | Connections and logins, see [Providers](providers.md) | |
| `--version` | Version and Codex compatibility | text |

Notes:

- A message starting with `-` must follow `--`.
- `--working-dir` is resolved to an absolute path that must exist; file tools
  are confined to it and hooks run in it.
- Statuses: `idle`, `running`, `failed` (the provider kept failing, or setup
  was wrong — see `termination_reason`), `terminated` (token cap reached or a
  hook blocked), `cancelled` (stopped). Any finished status accepts another `send`.
- A session's files live in `sessions/<id>/`: `metadata.json`, `events.jsonl`
  (append-only transcript; `tail -f` it), `prompt.md`, `logs.jsonl` (worker,
  retry and hook records), `worker.stderr.log`, and `queue.jsonl` while
  messages wait.

Typical loop:

```bash
ID=$(orchid create --agent worker --working-dir . | jq -r .id)
orchid send --id "$ID" -- "Fix the failing test and report what changed."
orchid await "$ID" --timeout 900 | jq -r '.sessions[0].message'
orchid send --id "$ID" -- "Now add a regression test."
```
