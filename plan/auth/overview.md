# Provider Authentication Framework

## Objective

Add a provider-neutral authentication layer to Orchid, supporting OpenAI API keys and a separate OpenAI Codex OAuth backend for ChatGPT-account access.

## Non-goals

- Do not treat ChatGPT Plus or Pro subscriptions as OpenAI Platform API credentials.
- Do not copy browser cookies, automate ChatGPT, or call private ChatGPT endpoints.
- Do not expose Codex OAuth tokens as normal API keys or send them to `api.openai.com/v1`.
- Defer OS keychain storage, MCP authentication, and ChatGPT-side integrations.

## Security principles

Credentials are user-managed references (environment variables or files), resolved only when a provider is constructed. Values must never appear in configuration inspection, session metadata, logs, policy hashes, command output, or provider errors. Missing or empty credentials fail before session or transcript mutation.

## Initial deliverable

Support named authentication profiles, shared by connections, with API-key references using `env.NAME` or `file./absolute/path`, plus an explicit `openai_codex` OAuth profile. Add `auth list`, `auth validate <name>`, and `auth login <name>` for Codex only.
