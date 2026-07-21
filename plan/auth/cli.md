# Authentication CLI

Add these commands under the selected config directory:

```text
orchid --config ./config auth list
orchid --config ./config auth validate openai-personal
orchid --config ./config auth login openai-codex
```

`auth list` reports profile names and non-secret type/reference metadata, with no credential values. `auth validate` confirms that the referenced environment variable or file exists and is non-empty; it never prints the credential and performs no provider request. A future explicit `--check` may perform provider verification.

`auth login <name>` is only valid for an `openai_codex_oauth` profile. It performs PKCE login, state-checked localhost callback handling, token exchange, account-claim validation, and secure access/refresh-token storage.

There is no generic subscription login: Codex OAuth is an explicit provider backend with provider-owned endpoints, scopes, refresh behavior, and model restrictions.
