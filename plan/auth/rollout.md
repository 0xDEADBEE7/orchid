# Authentication Implementation Phases

1. Add authentication model, resource loading, reference validation, and ephemeral resolution.
2. Update connection schema and effective configuration resolution; prevent legacy `api_key`/`auth` ambiguity.
3. Pass resolved credentials into provider constructors and remove provider-specific direct environment lookup.
4. Implement OpenAI API-key handling for environment and file references, including redacted and structured errors.
5. Implement the separate OpenAI Codex OAuth backend: PKCE, localhost callback, token exchange, secure persistence, refresh, claims, Codex transport, and model allowlisting.
6. Add `auth login` only for Codex profiles, plus inspection and validation commands.
7. Add the test coverage in `testing.md`, update checked-in examples and documentation, then run the full verification commands.

Persistent Codex token storage requires an explicit security design and restrictive permissions. Other OAuth providers remain separate future backends.
