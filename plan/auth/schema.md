# Authentication Schema and Validation

## Resources

Add an `auth` resource directory containing one JSON object per safe resource name. API-key profiles have a `value` reference; Codex OAuth profiles contain only their backend type, with tokens stored outside config.

Connections must reference an existing profile by name. File paths are not read while loading configuration files: loading validates shape and references only. Secret references are resolved by `auth validate` or immediately before runtime provider construction.

## Validation rules

- Auth profile names are safe resource names.
- Every connection `auth` reference exists.
- Authentication types are known.
- References are `env.NAME` or absolute `file.PATH` values.
- Secret-bearing fields reject literal values.
- OAuth token material is stored outside configuration with restrictive permissions.
- Environment variables are not resolved during file loading.
- `auth validate` rejects missing and empty values without printing them.
- Missing-secret errors identify only the variable name or file path, never a value.

Configuration inspection must redact authentication values and must not resolve them. Policy hashes and session metadata must contain profile names or other non-secret configuration only.
