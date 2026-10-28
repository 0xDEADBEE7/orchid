# Hooks

Hooks are scripts that run on session events. They are how policy is enforced:
there is no separate policy configuration.

```json
{"hooks": {"on-tool-call": [{"script": "hooks/guard.sh", "timeout": 10}]}}
```

Put `hooks` in `config.json` to run for every agent, and in `agents/<name>.json`
to run only for that agent's sessions. For each event the global hooks run
first, then the agent's, each in listed order. An agent can add hooks but cannot
remove or override global ones. A session keeps the copy of its agent's hooks
made at `create`; global hooks are always read live.

## Events

| Event | When |
|---|---|
| `on-event` | After any event is stored (user or assistant message, tool call, tool result, failure, termination). For a new user message, before it is stored. |
| `on-tool-call` | After a tool call is stored, before the tool runs. |
| `on-turn-end` | After a worker finishes a turn, whether it succeeded or failed. |

Filter `on-event` by `.event.event_type` (`message`, `tool_call`, `tool_result`,
`failure`, `termination`), e.g. `jq -e '.event.event_type == "failure"'`.

## Input

Each hook gets one JSON object on stdin:

```json
{"version": 1, "event": {"name": "on-tool-call", "event_type": "tool_call", "event_id": "...", "calls": [...]}, "session": {"metadata": {...}, "events": [...]}}
```

`event` is the stored event plus `name` and `event_type`; `session` is what
`orchid get <id>` prints. Environment: `ORCHID_CONFIG`, `ORCHID_SESSION_ID`,
`ORCHID_HOOK_EVENT`, `ORCHID_HOOK_DEPTH`. Hooks run in the session's working
directory (the config root if none). Script paths are relative to the config
root; a bare name not found there is looked up on `PATH`.

## Results

- Exit 0: continue.
- Exit 2: **block**, with stderr as the reason.
  - `on-tool-call` (or `on-event` for a tool call): the tool does not run; the
    model receives `{"error": "blocked by hook: <reason>"}` and can adjust.
  - `on-event` for a new user message: the message is not stored and `send`
    fails with the reason.
  - `on-event` for anything else during a turn: the turn stops and the session
    is terminated with the reason.
  - `on-turn-end`: no effect (the turn is over).
- Any other exit, or a timeout (default 30s; the whole process group is
  killed): logged in `sessions/<id>/logs.jsonl` and otherwise ignored. A crashing
  guard therefore allows the action, so make guards map their own errors to
  exit 2 (`trap 'exit 2' ERR` in bash).

Hooks run synchronously. For background work, detach it:
`long-task >/dev/null 2>&1 &`.

A hook may call the CLI, e.g. `orchid send --no-run --id "$ORCHID_SESSION_ID" "context"`.
While a turn is running, sends are queued and delivered before the next model
request; a `--no-run` message is delivered as context and never starts another
turn by itself. Nested hooks stop at depth 8.

## Policy examples

Allow file tools only under `src/`:

```bash
#!/bin/bash
trap 'echo "guard failed" >&2; exit 2' ERR
input=$(cat)
paths=$(jq -r '.event.calls[] | select(.name | startswith("fs_")) | .input.path // .input.paths[]?' <<<"$input")
for path in $paths; do
  case "$path" in src/*) ;; *) echo "file access outside src/: $path" >&2; exit 2 ;; esac
done
```

Refuse destructive shell commands:

```bash
#!/bin/bash
trap 'exit 2' ERR
cmd=$(jq -r '.event.calls[] | select(.name == "bash") | .input.cmd' </dev/stdin)
if grep -Eq 'rm -rf|git push --force' <<<"$cmd"; then
  echo "command not allowed: $cmd" >&2; exit 2
fi
```

Stop a session once it has made 200 requests (on `on-event`):

```bash
#!/bin/bash
requests=$(jq '.session.metadata.token_usage.requests' </dev/stdin)
[ "$requests" -lt 200 ] || { echo "request budget exhausted" >&2; exit 2; }
```

Tools a model may *use* are still listed in the agent's `tools`: hooks decide
whether a particular call may run, not which tools the model is offered.
