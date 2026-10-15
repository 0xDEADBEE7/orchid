
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

## Install

```bash
make build    # compiles to ./bin/orchid
```

`orchid --version` reports the Orchid release and the Codex protocol version it
advertises (the Astra gateway rejects outdated versions with HTTP 400).

## Quickstart

```bash
cp -R docs/resources/config ~/.config/orchid       # or pass --config DIR
orchid auth add local --base-url http://localhost:1234/v1 --model qwen3   # any OpenAI-compatible endpoint
ID=$(orchid create | jq -r .id)
orchid send --id "$ID" -- "Summarise this repository."
orchid await "$ID" | jq -r '.sessions[0].message'
tail -f ~/.config/orchid/sessions/$ID/events.jsonl | jq .   # stream events
```

## Guides

- [CLI reference](user-guide/cli.md)
- [Configuration](user-guide/configuration.md): agents, connections, prompts
- [Providers](user-guide/providers.md): `auth add` and base URLs for common providers
- [Hooks](user-guide/hooks.md): enforcing policy with scripts
- [Orchestration and workflows](user-guide/orchestration.md): sub-agents, background workers, scripted fan-out
- [Architecture](architecture.md)

## Astra smoke test

```bash
python3 scripts/smoke-astra.py [--binary ./bin/orchid] [--config DIR]
```

It uses the existing `codex-astra` connection and OAuth login, creates an
isolated configuration and session with tools and hooks disabled, sends one
request to `gpt-6-astra`, and requires the exact reply `ORCHID_ASTRA_SMOKE_PASS`.
Failures and timeouts exit nonzero; evidence stays in the printed temporary
directory. Authentication storage is linked, not copied, so an OAuth refresh
updates the existing login. No credentials are printed.
