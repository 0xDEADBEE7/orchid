# Target Architecture

## Resource boundaries

### Connection

A callable inference endpoint. It owns protocol/interface, URL, auth
reference, model identifier, provider parameters, and request headers. It must
not contain permissions, prompt composition, session state, or routing policy.

### Policy

An execution contract. It references one or more Connections in priority order,
contains limits and permissions, and describes prompt selection. It must not
contain mutable run state or copied conversation history.

### Prompt

A named Markdown document. Prompt loading and composition belong to the prompt
resolver, not the provider client. Missing prompts are configuration errors.

### Session

A durable unit of work. It owns identity, selected policy reference, prompt
reference/overrides, working directory, restrictions, transcript, state, and
log. A session must retain enough information to resume consistently after a
root default changes.

## Proposed module layout

```text
src/config/
  mod.rs          config-directory path and public facade
  root.rs         root config
  connection.rs   connection schema and loader
  policy.rs       policy schema and loader
  prompt.rs       prompt loader/composition
  resolve.rs      effective session configuration
src/session/
  mod.rs          session store and paths
  metadata.rs
  state.rs
```

`src/convo/` and the current `Profile`/`Config` types are removed rather than
retained as compatibility facades. Provider modules receive a resolved
Connection, never the whole root Config.

# Configuration directory selection

Every command in the new implementation accepts:

```text
--config <directory>
```

The directory contains `config.json`, `connections/`, `policies/`, `prompts/`,
and `sessions/`. The flag is propagated to detached `__run` processes so the
child resolves exactly the same resource graph as its parent.

When omitted, the new implementation uses the documented default config
location. During development, use a repository-local directory:

```bash
orchid --config ./config validate
orchid --config ./config create --policy default
orchid --config ./config send --await "hello"
```

`--config` selects a complete new configuration directory; it does not merge
with, inspect, or fall back to another directory.


The run loop should not independently reinterpret profile names, defaults, or
filesystem paths. Resolution occurs once at run setup and the result is passed
through the loop context.

## Immutability rule

A session references its selected policy and inputs. Existing sessions are not
changed by the root default changing. The policy hash provides an audit/version
signal; resolved effective configuration is intentionally not persisted.

## Error boundaries

- Missing root config: actionable initialization error.
- Missing named resource: include resource kind, name, and expected path.
- Invalid resource JSON: report path and schema field.
- Missing environment variable: report variable name without printing secret
  values.
- Provider errors: remain provider/runtime errors, not config parse errors.
