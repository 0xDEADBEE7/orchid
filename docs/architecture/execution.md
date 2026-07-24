# Execution

## Tool loop

`orchid send` appends the user message, starts the tool loop as a background process, and exits. The loop runs to completion independently:

1. Read `conversation.jsonl` to build message history.
2. Resolve the selected Policy, Connection candidates, and Prompt.
3. Send the prompt and message history to a provider through the `Provider` trait.
4. Stream and reduce the response; append message, reasoning, tool-call, and tool-result events to `conversation.jsonl`.
5. Execute tool calls and repeat, or finish on a final assistant message.

With `--await`, the calling process blocks until the loop completes. See [cli.md](cli.md).

---

## Run lifecycle

Run diagnostics are best-effort newline-delimited JSON records in
`orchid.log`. They are separate from the authoritative `conversation.jsonl`.
Run status and timestamps are persisted in `state.json`; metadata identity and
resource references remain in `metadata.json`. State and metadata updates use
atomic temporary-file writes and renames, but the diagnostic log and state/
metadata updates are not one atomic transaction.

At run start, `state.json` is updated to `status: "running"`, the process PID,
and `run_started_at`. On normal completion, failure, cancellation, or budget
termination, the lifecycle guard clears the PID and start time, sets the
terminal status, and records `last_run_at`.

A subsequent invocation checks a running session's stored PID. If the PID is
not alive, Orchid logs `run_crashed` to `orchid.log`, reconciles the state to
idle, and continues startup. Missing or invalid state is reported as a session
error rather than silently reconstructed. There is no `logs.jsonl` run-boundary
file or structured `run_id` event contract.

## Diagnostic records

`orchid.log` records fields such as `ts`, `level`, `event`, and `detail`. Events
include `run_start`, `run_end`, `run_crashed`, provider/tool diagnostics, and
budget or hook messages. The exact diagnostic stream is best-effort and is not
used to reconstruct conversation history or state.

---

## Built-in tools

| Tool | Description |
|------|-------------|
| `bash` | Execute a shell command. Path-validated against `working_dir` before execution. |
| `fs_read` | Read a file. Path-validated against `working_dir` before execution. |
| `fs_edit` | Replace an exact string in a file. Path-validated against `working_dir` before execution. |

### `bash` scope restriction

Before executing any command, orchid tokenises the command on whitespace and checks each token that looks like a path. Common shell expansions (`~`, `~/...`, `$HOME/...`, `${HOME}/...`) are expanded first. Each resulting absolute path is cleaned and checked against `working_dir`. If any path falls outside the working directory the command is not executed and a plain-string error is returned to the model as the tool result:

```
Error: path out of scope: /etc/hosts
```

This guards against accidental out-of-scope access and prompt injection attempting to reach sensitive paths. It does not prevent runtime path construction (e.g. paths computed during execution) — enforcement is static, applied to explicit tokens in the command string before the shell runs.

The subprocess is launched with its working directory set to `working_dir` so relative paths resolve naturally within scope.

`working_dir` is set in `metadata.json`. See [conversation.md](conversation.md).

---

## Observability

Events are written to `conversation.jsonl` as they occur. Observe in real time with standard tooling:

```bash
tail -f ./config/sessions/<id>/conversation.jsonl | jq .
```

Observe run state without log parsing:

```bash
cat ./config/sessions/<id>/state.json | jq .status
```

See [storage.md](storage.md) for path layout and [conversation.md](conversation.md) for file schemas.
