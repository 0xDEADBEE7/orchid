# Orchestrating agents

Use the orchestrator session to break a larger objective into explicit tasks,
delegate each task to a separate session, and use `await` to track completion.
An agent is not a continuation of the orchestrator: every session has its own
conversation and context. Include the task, relevant background, constraints,
working directory, and expected result in the delegated message. Agents cannot
see the orchestrator's conversation unless you provide that information.

## Send, await, inspect, and follow up

The orchestration loop is:

```text
send → await → get → inspect → follow up
```

A completed-agent response can be read without opening the config directory:

```bash
ID=$(orchid --config ./config send "Review the parser and report findings." | jq -r .id)
orchid --config ./config await "$ID" --timeout 300
orchid --config ./config get "$ID" --last-message
```

The final command returns the latest assistant message, for example:

```json
{"last_message":{"type":"message","message":{"role":"assistant","content":"The parser is correct; I found one missing edge-case test."}}}
```

Inspect that response, then use the same ID for a correction or next step:

```bash
orchid --config ./config send --id "$ID" \
  "Add the missing edge-case test, run the relevant Make target, and report the result."
orchid --config ./config await "$ID" --timeout 600
orchid --config ./config get "$ID" --last-message
```

`get` can read a running session too; it reports a point-in-time observation and
never waits or changes session state. Use `--conversation` when the complete
ordered transcript is needed. `--last-message` selects the latest assistant
`message` event, rather than the final JSONL line (which may be a tool result).
All reads stay within the selected `--config` session store.


Use sequential delegation when a later task depends on an earlier result:

```bash
ID=$(orchid --config ./config send \
  "Inspect the project. Identify the three highest-priority test failures. Return file paths and a short plan." \
  | jq -r .id)

orchid --config ./config await "$ID" --timeout 300

# Inspect the completed session's transcript, then delegate the next task.
orchid --config ./config send \
  --id "$ID" \
  "Using the findings from your previous task, implement the plan. Run the relevant tests and report any remaining failures."
orchid --config ./config await "$ID" --timeout 600
```

For independent sequential tasks, create a new session for each checklist item
and await it before starting the next one. Pass forward any findings explicitly;
do not assume the next agent knows what the previous agent did.

## Follow up with an agent

A terminal session is still available for follow-up. If an agent fails,
misunderstands the task, or reports incomplete work, send another message to the
same session with `--id`. The agent then receives its existing session history
as context. Explain what went wrong and give concrete corrective guidance:

```bash
ID="<session-id>"

orchid --config ./config send --id "$ID" \
  "The implementation is incomplete: the parser was updated, but its tests are missing. Add focused tests for invalid input and timeout behavior. Run the relevant test target and report the result."
orchid --config ./config await "$ID" --timeout 600
```

After awaiting, inspect the new response and the repository before marking the
task complete. Follow-up messages are useful for correction and iteration, but
they do not make an unsuccessful first attempt successful automatically. The
orchestrator remains responsible for reviewing the diff and validation results.

### Read the last assistant message

The transcript is stored at
`./config/sessions/<SESSION_ID>/conversation.jsonl`. To print the complete last
assistant message, use `jq` to select assistant messages, take the last object,
and then extract its content:

```bash
ID="<session-id>"
jq -rs '
  map(select(.type == "message" and .message.role == "assistant"))
  | last
  | .message.content
' "./config/sessions/$ID/conversation.jsonl"
```

A shorter version is suitable when the response is known to be one line:

```bash
jq -r 'select(.type == "message" and .message.role == "assistant") | .message.content' \
  "./config/sessions/$ID/conversation.jsonl" | tail -1
```

The shorter `tail -1` form returns only the final line of a multiline response,
not the complete message. Prefer the first form for orchestration. The session's
`state.json` also contains a `last_message` summary, but the transcript is the
source for the complete response.
## Parallel workflow

Run independent tasks at the same time, then await their IDs together:

```bash
IDS=()
IDS+=("$(orchid --config ./config send \
  "Review the authentication code for correctness and security issues. Report findings with file paths." \
  | jq -r .id)")
IDS+=("$(orchid --config ./config send \
  "Review the test suite. Identify missing coverage and propose specific tests." \
  | jq -r .id)")

orchid --config ./config await "${IDS[@]}" --timeout 600 --interval 2
```

`await` returns when one or more sessions reach a terminal state. Remove the
returned IDs from your pending list, process their results, and await the
remaining IDs:

```bash
result=$(orchid --config ./config await "${IDS[@]}" --timeout 600)
completed=$(jq -r '.completed[].id' <<<"$result")

remaining=()
for id in "${IDS[@]}"; do
  grep -qxF "$id" <(printf '%s\n' "$completed") || remaining+=("$id")
done
[ "${#remaining[@]}" -eq 0 ] || orchid --config ./config await "${remaining[@]}" --timeout 600
```

Only parallelize tasks that cannot conflict. Give agents separate working
copies or directories when they may edit overlapping files. If a later task
needs an earlier result, await and inspect the first result before launching it.

## Delegation checklist

For each task, provide:

- one concrete objective and a clear definition of done;
- relevant files, paths, prior findings, and commands to use;
- constraints such as scope, style, or files not to change;
- expected output, including tests run and unresolved issues;
- the session ID and working directory to use for follow-up work.

Treat `idle`, `failed`, and `cancelled` as terminal outcomes. `await` observes
sessions; it does not cancel or repair them. A failed or cancelled result still
needs to be reviewed before the checklist item can be marked complete.
