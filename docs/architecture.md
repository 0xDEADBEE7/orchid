# Architecture

Orchid is a CLI over a directory of sessions. There is no daemon.

## Sessions and storage

`sessions/<id>/metadata.json` holds status, worker pid, token usage, the
session's own copy of its agent (`agent`: connection, tools, max_tokens, hooks)
and its parent, if any. `events.jsonl` is the append-only transcript: user and
assistant messages, tool calls and results, failures and terminations. Writers
take an `flock` on `.flock`; metadata is written via a temporary file and
rename. Existing transcript lines are never rewritten.

## A turn

`send` stores the user message (after running its hooks) and claims the session
in one locked step, then spawns a detached worker (`orchid __run --id ID`) in
its own process group. The worker loops:

1. deliver queued messages (`queue.jsonl`), check the token cap;
2. request a reply from the connection's client, retrying transient failures
   (network errors, HTTP 408/429/5xx except 501/505, Codex `server_error` /
   `server_is_overloaded`) after 2, 4 and 8 seconds;
3. store any text, then run each tool call: store it, fire `on-event` and the
   `on-tool-call` gate, run the tool (or return the block reason), store the
   result (capped at 50K characters);
4. repeat until a reply has no tool calls.

At the end of a turn the worker atomically either picks up queued messages and
runs another turn, or marks the session idle; then `on-turn-end` fires. Errors
become a `failure` (or `termination`) event with the reason in
`termination_reason`. `await` and `send` reconcile sessions whose worker died.

## Code map

| Path | Role |
|---|---|
| `src/main.rs`, `src/cli_*.rs` | command dispatch, `send`, `await`/`stop`, argument helpers |
| `src/config.rs`, `src/agent.rs`, `src/config/auth_command.rs` | config root, agents, connections, credentials, `auth` |
| `src/model.rs` | sessions, events, metadata |
| `src/store.rs`, `src/store/queue.rs` | locked persistence and the message queue |
| `src/provider/` | the turn loop, retries, tool dispatch, the worker |
| `src/client/` | OpenAI-compatible chat and Codex Responses clients, shared error classification |
| `src/hooks.rs` | hook execution and the exit-2 rule |
| `src/tools.rs` | `bash`, `fs_read`, `fs_edit`, output cap |
