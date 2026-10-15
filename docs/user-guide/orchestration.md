# Orchestration and workflows

Every agent runs in its own session. Sessions are the sub-agent primitive; there
are two ways to coordinate them.

## Chatting with an orchestrator (background sub-agents)

An orchestrator agent with the `bash` tool starts workers itself and keeps
talking to you while they run:

1. It creates a worker session and sends it a task. `send` returns immediately.
   Inside a session's tools, `$ORCHID_BIN`, `ORCHID_CONFIG` and
   `ORCHID_SESSION_ID` are set, and `create` records the creating session as
   the new session's `parent`.
2. It ends its turn, so it is idle and you can keep chatting.
3. When the worker's turn ends, the template's global `on-turn-end` hook
   (`hooks/notify-parent.sh`) sends the parent a message:
   `<subagent id="…" status="idle">worker's last message</subagent>`. If the
   orchestrator is idle this starts a turn so it can react; if it is busy the
   message is queued.

Messages sent to a busy session are queued (`send` prints
`{"status":"queued"}`) and delivered before the worker's next model request, or
right after its current turn, so you can always send.

The orchestrator prompt in `docs/resources/config/prompts/orchestrator.md`
(regenerate with `make prompts` from `docs/orchestrator/`) follows this model.

## Scripted workflows

For deterministic fan-out (Claude Code-style workflows), drive sessions from a
script. `await` is the blocking primitive:

```bash
orchid await ID... [--timeout SECONDS]   # default 60
```

It prints `{"sessions":[{"id","status","message","reason"}]}` once every session
has finished, where `message` is the last assistant message of idle sessions and
`reason` explains a failure. Exit status: 0 all idle, 1 if any failed, was
cancelled or was terminated, 2 on timeout.

A minimal Python helper:

```python
import json, subprocess
from concurrent.futures import ThreadPoolExecutor

def orchid(*args):
    out = subprocess.run(["orchid", *args], capture_output=True, text=True)
    return json.loads(out.stdout or out.stderr)

def agent(prompt, agent="worker", schema=None, working_dir=None):
    """Runs one sub-agent to completion; returns its reply (parsed if schema)."""
    extra = ["--working-dir", working_dir] if working_dir else []
    sid = orchid("create", "--agent", agent, *extra)["id"]
    if schema:
        prompt += "\nReply with only JSON matching this schema:\n" + json.dumps(schema)
    for attempt in range(2):
        orchid("send", "--id", sid, "--", prompt)
        result = orchid("await", sid, "--timeout", "3600")["sessions"][0]
        if result["status"] != "idle":
            return None
        if not schema:
            return result["message"]
        try:
            return json.loads(result["message"])
        except (TypeError, ValueError):
            prompt = "That was not valid JSON. Reply with only the JSON."
    return None

def parallel(*thunks):
    with ThreadPoolExecutor() as pool:
        return list(pool.map(lambda thunk: thunk(), thunks))

# pipeline: plain composition; loop-until-dry: a while loop over agent().
reviews = parallel(*(lambda f=f: agent(f"Review {f} for bugs.") for f in ["a.rs", "b.rs"]))
summary = agent("Summarise these reviews:\n" + "\n".join(filter(None, reviews)))
```

Give parallel workers separate working directories (e.g. git worktrees) when
they may edit files. To run a workflow from an orchestrator without blocking
it, start the script detached (`python flow.py > flow.log 2>&1 &`) and have it
finish with `"$ORCHID_BIN" send --id "$ORCHID_SESSION_ID" -- "<result>"`.

## Inspecting and stopping

- `orchid get ID --last-message` prints the latest assistant message;
  `orchid get ID` prints the whole session. `tail -f
  sessions/ID/events.jsonl` streams events; `sessions/ID/logs.jsonl` shows
  retries and hook results.
- `orchid stop ID` ends the worker and its tool commands, answers open tool
  calls, and marks the session `cancelled`. Queued messages are kept and
  delivered by the next `send`.
- Statuses: `running`, `idle`, `failed` (e.g. the provider kept failing),
  `terminated` (token cap, or a hook blocked), `cancelled`.
