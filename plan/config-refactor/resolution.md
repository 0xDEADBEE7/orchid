# Resolution Contract

## Precedence

Resolve a new session in this order:

1. Explicit CLI policy/session option.
2. Root `config.json` `policy`.
3. Legacy adapter, if no new resource tree exists.
4. Fail with an initialization error.

Within a policy, connection selection is the declared ordered list. The first
usable connection is selected; retries must remain within the policy's list and
be observable in diagnostics.

## Effective configuration

Introduce an owned value such as:

```rust
pub struct EffectiveSessionConfig {
    pub policy_name: String,
    pub policy_hash: String,
    pub connection_candidates: Vec<Connection>,
    pub prompt: String,
    pub working_dir: PathBuf,
    pub permissions: Permissions,
    pub limits: Limits,
    pub env_vars: HashMap<String, String>,
}
```

The session stores the selected policy identity and hash as metadata. Effective
connections, prompts, permissions, limits, and environment values are resolved
just in time for each run and are not persisted as a session snapshot.

## Scope and permissions

Preserve policy permissions as the maximum allowed tools and paths. Session
restrictions may only reduce access. Add tests proving a session cannot expand
policy paths or tools.

## Environment resolution

Centralize `env.<VAR>` handling for connection keys, headers, and env files.
Resolution must happen at request/run time, not while writing resource files.
Errors distinguish unset variables from literal values. Avoid logging values;
existing debug logs should continue to log only presence and names.

## Prompt resolution

Load the named prompt relative to the Orchid config directory. If composition
is supported, load fragments in declared order, normalize separators once, and
record the prompt name(s) and content hash in session metadata/state.

## Reload behavior

- The session stores the selected policy identity and hash; resolved
  connections, prompts, limits, permissions, and environment values are loaded
  just in time for each run.
- A running loop uses the effective configuration resolved at run setup and does
  not change when files are edited mid-run.
- A later run may resolve updated resources from the same policy identity.
- `config use` becomes a root-policy update and must use atomic replacement.
