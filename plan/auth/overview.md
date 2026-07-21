# Provider Authentication Framework

## Objective

Add a provider-neutral authentication layer to Orchid, initially supporting OpenAI API keys while leaving a clean boundary for future officially supported OAuth or subscription providers.

## Non-goals

- Do not treat ChatGPT Plus or Pro subscriptions as API credentials.
- Do not copy browser cookies, automate ChatGPT, or call private ChatGPT endpoints.
- Do not add a generic `auth login` command before a provider publishes an official flow.
- Defer OS keychain storage, MCP authentication, and ChatGPT-side integrations.

## Security principles

Credentials are user-managed references (environment variables or files), resolved only when a provider is constructed. Values must never appear in configuration inspection, session metadata, logs, policy hashes, command output, or provider errors. Missing or empty credentials fail before session or transcript mutation.

## Initial deliverable

Support named authentication profiles, shared by connections, with API-key references using `env.NAME` or `file./absolute/path`. Add `auth list` and `auth validate <name>`; validation checks presence and non-emptiness without making a provider request.
