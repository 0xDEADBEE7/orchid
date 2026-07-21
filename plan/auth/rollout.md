# Authentication Implementation Phases

1. Add authentication model, resource loading, reference validation, and ephemeral resolution.
2. Update connection schema and effective configuration resolution; prevent legacy `api_key`/`auth` ambiguity.
3. Pass resolved credentials into provider constructors and remove provider-specific direct environment lookup.
4. Implement OpenAI API-key handling for environment and file references, including redacted and structured errors.
5. Add `auth list`, `auth validate`, inspection redaction, and command/help integration.
6. Add the test coverage in `testing.md`, update checked-in examples and documentation, then run the full verification commands.

Persistent token storage and provider-specific OAuth are later phases, gated on an official provider flow and an explicit security design.
