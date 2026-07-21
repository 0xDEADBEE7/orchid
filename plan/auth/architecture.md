# Authentication Architecture

## Configuration relationship

Connections select a named profile:

```json
{
  "interface": "openai",
  "base_url": "https://api.openai.com",
  "model": "gpt-5",
  "auth": "openai-personal"
}
```

Profiles contain a kind and a secret reference, not a secret database:

```json
{
  "type": "api_key",
  "value": "env.OPENAI_API_KEY"
}
```

The resolver returns an ephemeral `ResolvedCredential { kind, value }` during provider construction. Provider-specific code receives this resolved pair and must not resolve process environment variables itself.

## Types and boundaries

Introduce `AuthProfile`, `AuthKind` (`ApiKey`, `BearerToken`, `OpenAiCodexOAuth`), and a resolver supporting:

- `env.NAME`: resolve from the process environment at validation/runtime time.
- `file./absolute/path`: read a user-managed file and remove only its final newline.

Literal secret values are invalid. Unknown authentication kinds and malformed references are configuration errors. OpenAI-compatible local endpoints continue to use the existing connection shape and API-key transport.

## Provider behavior

OpenAI API-key authentication talks to the OpenAI Platform API. ChatGPT-account access is a separate Codex adapter using Sign in with ChatGPT/Codex OAuth, PKCE, refresh tokens, localhost callback, account claims, and the Codex backend endpoint. It is not ordinary OpenAI API authentication.

## Compatibility

Adopt `auth` as the connection authentication field. Legacy `api_key` must not coexist with `auth`; retain it only for a narrowly scoped transition if implementation constraints require it, and update checked-in examples accordingly.
