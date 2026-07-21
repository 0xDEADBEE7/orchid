# CLI

## Output contract

All commands write JSON to stdout. This applies universally — including errors.

```bash
# single object
{"id":"a3f9c1b2...","label":"fix-auth-bug","created_at":"..."}

# list
[{"id":"a3f9c1b2...","label":"fix-auth-bug"},{"id":"d7e2a091...","label":"add-tests"}]

# error
{"error":"policy not found","policy":"missing-policy"}
```

No human-readable formatting is ever the default. Pipe through `jq` for filtering and display.

Streaming conversation output is intentionally not provided by the CLI — use standard tooling directly:

```bash
tail -f ./config/sessions/<id>/conversation.jsonl | jq .
```

Every command in the new configuration model accepts `--config <directory>`.
The directory is propagated to detached background runs and is the complete
resource/state boundary for that command.

```bash
orchid --config ./config validate
orchid --config ./config send --await "message"
```

See [storage.md](storage.md) for the full path layout.

---

## ID vs label

Conversations have two identifiers:

| | `id` | `label` |
|---|------|---------|
| Format | 32-char hex (e.g. `a3f9c1b2...`) | Human-readable string (e.g. `fix-auth-bug`) |
| Assigned | At creation — immutable | At creation or any time — mutable |
| Unique | Yes | No |
| Purpose | Stable reference for scripts | Human annotation only |

`--id` flags accept the hex ID only. Labels are surfaced in `orchid list` output for reference; use `jq` to look up the ID from a label before passing it to a command.

```bash
ID=$(orchid list | jq -r '.[] | select(.label == "fix-auth-bug") | .id')
```

---

## Commands

### `orchid send`

Append a user message and start the tool loop. Returns immediately after writing
the user message and updating session state; the loop runs as a background
process.

```bash
orchid --config ./config send --id <id> "message"
```

Options:

| Flag | Description |
|------|-------------|
| `--config` | Complete configuration and session directory |
| `--id` | Session hex ID |
| `--policy` | Override the root policy for a new run |
| `--await` | Block until the turn completes |

---

### `orchid list`

List resources. Returns a JSON array in all cases.

```bash
orchid --config ./config list
orchid --config ./config list connections
orchid --config ./config list policies
orchid --config ./config list prompts
```

See [conversation.md](conversation.md) for the session metadata schema and
[config.md](config.md) for resource schemas.

---

### `orchid set`

Mutate persistent session settings.

```bash
orchid --config ./config set --id <id> --label <name>
orchid --config ./config set --id <id> --working-dir <path>
orchid --config ./config set --id <id> --policy <name>
```

All changes are written to the session metadata/state files.

---

### `orchid config`

Manage the selected configuration directory.

```bash
orchid --config ./config validate
orchid --config ./config config path
orchid --config ./config config current
```

`validate` checks the root config and every referenced Connection, Policy, and
Prompt before a run. There is no profile or legacy configuration command.
