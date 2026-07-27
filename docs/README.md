
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
- Anthropic and OpenAI-compatible providers, including the Codex OAuth client
