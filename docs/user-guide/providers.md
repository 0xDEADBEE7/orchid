# Providers

Add a connection with one command. On a terminal, missing values are prompted
for; in scripts, pass them as flags. The command writes
`connections/<name>.json`, makes it the default agent's connection if that has
none, and sends one test request.

```bash
orchid auth add <name> --base-url URL --model MODEL [--key-env VAR] [--header KEY=VALUE]...
orchid auth test <name>      # send one test request through an existing connection
orchid auth list             # connections and Codex logins
orchid auth login <name>     # Codex (ChatGPT) OAuth; prints the sign-in URL
```

Keys are never stored: `--key-env OPENAI_API_KEY` records `"api_key":
"env.OPENAI_API_KEY"`, so the variable must be set wherever `orchid send` (and
therefore the worker) runs. For a key kept in a file, edit the connection to
`"api_key": "file./absolute/path"`. Header values may also be `env.NAME` or
`file.PATH`.

| Provider | `--base-url` | `--key-env` |
|---|---|---|
| OpenAI | `https://api.openai.com/v1` | `OPENAI_API_KEY` |
| Anthropic (OpenAI-compatible endpoint) | `https://api.anthropic.com/v1` | `ANTHROPIC_API_KEY` |
| OpenRouter | `https://openrouter.ai/api/v1` | `OPENROUTER_API_KEY` |
| Groq | `https://api.groq.com/openai/v1` | `GROQ_API_KEY` |
| Together | `https://api.together.ai/v1` | `TOGETHER_API_KEY` |
| LM Studio | `http://localhost:1234/v1` | — |
| Ollama | `http://localhost:11434/v1` | — |
| Azure OpenAI (v1 API) | `https://<resource>.openai.azure.com/openai/v1` | add `--header api-key=env.AZURE_OPENAI_API_KEY` |
| Company gateway (Portkey, LiteLLM, …) | the gateway's OpenAI-compatible URL | as required, plus `--header` for gateway headers |

A base URL ending in `/v1`, or a bare host, gets `/chat/completions` (or
`/v1/chat/completions`) appended. A full URL containing `/chat/completions`,
including a query string such as Azure's `?api-version=…`, is used as given.

Example for a gateway that needs its own headers:

```bash
orchid auth add work --base-url https://gateway.example.com/v1 --model claude-sonnet \
  --key-env GATEWAY_KEY --header x-portkey-provider=anthropic --header x-team=platform
```

Codex: `orchid auth login codex`, then a connection
`{"interface": "codex", "base_url": "https://chatgpt.com/backend-api/codex", "model": "…", "auth": "codex"}`
(the template ships one).

Request parameters (e.g. `"temperature"`, `"reasoning_effort"`) go in the
connection's `"params"` and are merged into every request body.
