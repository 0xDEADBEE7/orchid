
# ORCHID

---

             __  '▄▄▄▄▄▄'  __
            '  `\▜██████▛/`  '
          __▗▟████▙▜██▛▟████▙▖__
         ' :███████▌##▐███████: '
             ▜████▛▟██▙▜████▛:
             '▝▀▀:▟████▙:▀▀▘'
                   ▀▀▀▀
                    
---

> Headless, composable CLI for LLM conversations.

See [user-guide/](user-guide/index.md) for the full usage reference and [architecture/](architecture/index.md) for design documentation.

## Install

```bash
make build    # compiles to ./bin/orchid
```

`orchid --version` reports the Orchid release and Codex compatibility version.
Orchid 0.1.1 advertises Codex 0.153.4 in both HTTP clients; the previous 0.144.4
headers caused the Astra gateway to reject requests with HTTP 400. Gateway
errors now include a bounded response body and the request ID when available.

### Astra smoke test

After installing the release binary, run:

```bash
python3 scripts/smoke-astra.py
# Or test a specific build:
python3 scripts/smoke-astra.py --binary ./bin/orchid
```

The script uses the existing `codex-astra` connection and OAuth login from
`~/.config/orchid` (override with `--config`). It creates an isolated configuration
and session, disables tools and hooks, sends a small live request to `gpt-6-astra`,
and requires the exact response `ORCHID_ASTRA_SMOKE_PASS`. Failures and timeouts
produce a nonzero exit status. Evidence is retained in the printed temporary
directory. Authentication storage is linked, not copied, so any OAuth refresh
updates the existing login. No credentials are printed.

## Usage

```bash
# validate and select the repository-local resource tree
orchid --config ./config config validate
orchid --config ./config config use default

# send a message (non-blocking — returns id immediately)
ID=$(orchid send "fix the failing test" | jq -r .id)

# send and block until complete
orchid send --id $ID --await "fix the failing test"

# continue an existing conversation
orchid send --id <id> "follow up message"

# stream events in real time
tail -f ./config/sessions/<id>/conversation.jsonl | jq .

# inspect a completed session through the CLI
orchid --config ./config get <id> --last-message
orchid --config ./config get <id> --conversation \\
  | jq --argjson n 10 '.conversation | .[-$n:]'

# check run state
jq .status ./config/sessions/<id>/metadata.json
```
## Resource template

A clean baseline configuration tree is maintained in [`../resources/`](../resources/).
It is based on the declarative resources in `.test-config/`, with runtime
sessions and pre-filled authentication excluded.

Copy it to `./config/` before selecting it:

```bash
cp -R resources ./config
orchid --config ./config config validate
```

Add credentials separately with environment-backed references or
`orchid auth login` after copying it. Runtime session data is created under
`./config/sessions/` and should not be added to the template.

## Design
- Tool loop execution: read the conversation transcript → call the model → execute tools → append results → repeat
- Stream-first: observe with `tail -f` and standard tooling
- OpenAI-compatible providers (including Anthropic's compatibility endpoint) and the Codex OAuth client
