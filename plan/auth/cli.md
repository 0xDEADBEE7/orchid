# Authentication CLI

Add these commands under the selected config directory:

```text
orchid --config ./config auth list
orchid --config ./config auth validate openai-personal
```

`auth list` reports profile names and non-secret type/reference metadata, with no credential values. `auth validate` confirms that the referenced environment variable or file exists and is non-empty; it never prints the credential and performs no provider request. A future explicit `--check` may perform provider verification.

Do not implement `auth login` yet. Official OAuth requires provider-specific authorization endpoints, client registration, scopes, redirect or device behavior, token exchange, refresh, expiry errors, and consent policy. Such a backend must be added separately when a provider officially supports it.
