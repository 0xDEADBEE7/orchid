# Authentication Test Plan

Cover the following with unit and integration tests:

- Environment-backed API-key resolution.
- File-backed API-key resolution, including final-newline trimming.
- Missing environment variables and files, with no secret leakage.
- Empty credential rejection.
- One profile shared by multiple connections.
- Invalid and missing profile references.
- `config show` redaction and no secret-bearing policy/session metadata.
- Provider errors and HTTP logs never containing API keys.
- Authentication failure before session or transcript creation.
- OpenAI requests using the resolved credential.
- Structured unsupported ChatGPT-subscription authentication errors.
- Codex OAuth PKCE state/callback validation, token refresh, secure storage, and account-ID extraction.
- Codex requests use the Codex endpoint and never the standard API endpoint.
- Isolation between two config directories.

Verification commands:

```bash
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all
```

Any existing macOS `reqwest` `system-configuration` panic is tracked separately from authentication work if it remains reproducible.
