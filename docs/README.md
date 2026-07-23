
               _    ▄▄    _ 
              ' `\▟████▙/` '
             ▗▟███▙▜██▛▟███▙▖
             ' ▀▜██▌::▐██▛▀ '
               ▟██▛▟██▙▜██▙
              :▝▀▀▘▜██▛▝▀▀▘:
                    ''
---

# orchid

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
jq .status ./config/sessions/<id>/state.json
```

## Design

- 1 session = 1 directory under the selected config directory's `sessions/`
- Tool loop execution: read log → call model → execute tools → append results → repeat
- Stream-first: observe with `tail -f` and standard tooling
- Anthropic provider only
