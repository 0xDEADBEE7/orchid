# Providers

Run `orchid auth add` in a terminal for a guided setup:

1. pick a provider: OpenAI, ChatGPT subscription (Codex sign-in), Anthropic
   Claude (API key), OpenRouter, Groq, Together, LM Studio, Ollama, Azure
   OpenAI, or a custom OpenAI-compatible endpoint (company gateway, LiteLLM,
   vLLM, ...);
2. name the connection and confirm or change the base URL;
3. give the environment variable holding your API key. If it isn't set, you
   can paste the key instead (input hidden); it is stored in
   `auth/<name>.key`, readable only by you;
4. choose a model (the endpoint's model list is shown when it offers one) and,
   for custom endpoints and Azure, any extra headers;
5. decide whether the default agent should use it. A test request follows.

The ChatGPT subscription option opens a browser to sign in and stores the login
in `auth/tokens/<name>.json`. Claude Pro/Max subscriptions can't be used:
Anthropic only allows them in its own apps (claude.ai, Claude Code), so use an
Anthropic API key, or run Claude Code itself.

Every prompt has a flag, so scripts can run the same command without a terminal:

```bash
orchid auth add [NAME] [--provider P] [--base-url URL] [--model MODEL] [--key-env VAR] [--header KEY=VALUE]... [--default]
orchid auth test <name>      # send one test request through an existing connection
orchid auth list             # connections and Codex logins
orchid auth login <name>     # sign in again with a ChatGPT subscription
```

Providers (`--provider`): `openai`, `codex`, `anthropic`, `openrouter`, `groq`,
`together`, `lmstudio`, `ollama`, `azure`, `custom` (the default without a
terminal). A provider's key variable is only suggested for its own base URL.
Keys are never written to connection files: `"api_key"` holds `env.NAME` (the
variable must be set wherever `orchid send` runs) or `file.PATH`. Header values
may also be `env.NAME` or `file.PATH`.

| Provider | `--base-url` | `--key-env` |
|---|---|---|
| OpenAI | `https://api.openai.com/v1` | `OPENAI_API_KEY` |
| Anthropic (OpenAI-compatible endpoint) | `https://api.anthropic.com/v1` | `ANTHROPIC_API_KEY` |
| OpenRouter | `https://openrouter.ai/api/v1` | `OPENROUTER_API_KEY` |
| Groq | `https://api.groq.com/openai/v1` | `GROQ_API_KEY` |
| Together | `https://api.together.ai/v1` | `TOGETHER_API_KEY` |
| LM Studio | `http://localhost:1234/v1` | — |
| Ollama | `http://localhost:11434/v1` | — |
| Azure OpenAI (v1 API) | `https://<resource>.openai.azure.com/openai/v1` | `AZURE_OPENAI_API_KEY` (sent as the `api-key` header) |
| Company gateway (Portkey, LiteLLM, …) | the gateway's OpenAI-compatible URL | as required, plus `--header` for gateway headers |

For OpenAI-compatible clients, a base path ending in `/v1` gets
`/chat/completions` appended; other base paths get `/v1/chat/completions`.
Codex appends `/responses`. Full endpoint paths ending in the client's expected
endpoint (with an optional trailing slash) are used as given. Query strings
are preserved, and appended paths go before the query string.

A full `/responses`, `/chat/completions`, or `/messages` endpoint belonging to
a different API is rejected as a configuration error. Setting the URL does not
switch request formats: the OpenAI-compatible client uses Chat Completions;
Codex uses Responses.

Example for a gateway that needs its own headers:

```bash
orchid auth add work --base-url https://gateway.example.com/v1 --model claude-sonnet \
  --key-env GATEWAY_KEY --header x-portkey-provider=anthropic --header x-team=platform
```

Request parameters (e.g. `"temperature"`, `"reasoning_effort"`) go in the
connection's `"params"` and are merged into every request body.
