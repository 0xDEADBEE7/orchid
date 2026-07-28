


# Orchid Configuration & Storage Architecture

## Overview

Orchid separates **configuration** from **persisted work**.

Configuration defines **how Orchid should behave**.

Sessions represent **work performed by Orchid** and contain all information required to pause, resume and continue that work.

The architecture is intentionally composed of a small number of first-class resource types.

| Resource   | Purpose                                                                                          |
| ---------- | ------------------------------------------------------------------------------------------------ |
| Connection | Defines a callable LLM endpoint.                                                                 |
| Policy     | Defines how Orchid executes a session, including model routing, permissions and execution rules. |
| Prompt     | Defines reusable system prompts.                                                                 |
| Session    | Represents a persisted unit of work including conversation history and execution state.          |

Each resource exists independently and may be shared, version controlled and reused.

---

# High Level Filesystem Layout

```text
~/.config/orchid/

config.json

connections/
policies/
prompts/

sessions/
```

Each directory contains a homogeneous collection of resources.

Adding new behaviour should typically involve dropping a new file into one of these directories rather than modifying existing configuration.

This keeps configuration modular, portable and easy to version.

---

# Root Configuration

```text
config.json
```

The root configuration acts as the entry point into the remainder of the configuration graph.

It intentionally remains small.

Rather than embedding configuration, it specifies which reusable resources should be used by default when creating new sessions.

Example:

```json
{
    "policy": "default",
    "hooks": {
        "turn-start": ["./scripts/prepare.sh"],
        "turn-stop": ["./scripts/notify.sh"]
    }
}
```

`hooks` is optional and supports the `turn-start` and `turn-stop` lifecycle
events. See [hooks.md](hooks.md) for its execution and payload contract.

Future versions may also contain:

* global application settings
* search paths
* plugin locations
* feature flags

The root configuration should never become a monolithic configuration file.

---

# Connections

Directory:

```text
connections/
```

A Connection represents a complete inference endpoint.

It encapsulates everything required for Orchid to communicate with a language model regardless of provider.

Supported connection interfaces currently include:

* OpenAI-compatible APIs (`openai`)
* Anthropic Messages API (`anthropic`)

An `openai` connection referencing an authentication profile whose
`type` is `"openai_codex_oauth"` selects the Codex OAuth client. Other
interfaces are rejected during provider creation.

A connection contains:

* interface/protocol
* endpoint URL
* authentication
* model identifier
* provider-specific parameters

Example:

```json
{
    "name": "local-fast",

    "interface": "openai",

    "base_url": "http://localhost:1234",

    "api_key": "dummy",

    "model": "qwen/qwen3.6-35b-a3b",

    "params": {
        "reasoning_effort": "none",
        "max_tokens": 4096
    }
}
```

Connections intentionally hide provider-specific implementation details behind a common abstraction.

The routing layer never needs to understand individual provider APIs.

---

# Policies

Directory:

```text
policies/
```

A policy contains an ordered list of connection resource names and an
`active_connection` zero-based index. The indexed entry is the connection used
for the session; the list order is not a fallback order.

```json
{
    "active_connection": 0,
    "connections": ["echo", "codex"],
    "permissions": {
        "tools": ["bash", "fs_read", "fs_edit"],
        "paths": ["/tmp/**"]
    }
}
```

The index is part of the session's agent policy snapshot, so changing the
configuration does not change an existing session's active connection.

---

# Prompts

Directory:

```text
prompts/

base.md
engineering.md
audit.md
...
```

Prompts are reusable Markdown documents.

Sessions reference prompts by name.

Prompt composition may be expanded in future revisions.

---

# Sessions

Directory:

```text
sessions/

<session-id>/
```

A Session represents a persisted unit of work.

Unlike a traditional runtime session, Orchid sessions are durable.

A session may be paused, resumed, transferred between machines and continued at any point in the future.

A session owns all information required to continue execution.

Example:

```text
sessions/

8372ccf911fe59c4c4e33ae0a4190836/

    conversation.jsonl

    metadata.json

    state.json

    orchid.log
```

---

# Conversation

```text
conversation.jsonl
```

Contains the complete chronological transcript.

The conversation is append-only.

Entries may include:

* user messages
* assistant messages
* tool calls
* tool results
* future event types

The conversation is considered the authoritative record of interaction.

---

# Session Metadata

```text
metadata.json
```

Contains descriptive information about the session.

Example:

```json
{
    "id": "...",

    "label": "Engineering",

    "created_at": "...",

    "working_directory": "...",

    "prompt": "engineering"
}
```

Metadata describes the identity of the session rather than its current execution state.

Unlike runtime state, metadata changes infrequently.

---

# Session State

```text
state.json
```

Contains mutable information associated with the session.

Examples include:

```json
{
    "status": "idle",

    "token_estimate": 18452,

    "last_run_at": "...",

    "run_started_at": "...",

    "permissions": {
        ...
    }
}
```

Sessions may apply additional restrictions on top of the selected policy.

Session permissions may never expand beyond those granted by the policy.

Instead, they represent additional constraints that apply only to that session.

---

# Logging

```text
orchid.log
```

Contains diagnostic information relating to the session.

Logs are intentionally separated from the conversation transcript to avoid polluting conversational history.

---

# Configuration Resolution

When a new session is created, Orchid resolves configuration using the following chain:

```text
config.json
        │
        ▼
 Default Policy
        │
        ▼
 Session Metadata
        │
        ▼
 Session Overrides
        │
        ▼
Effective Session Configuration
```

The resulting effective configuration governs execution for the lifetime of the session.

Changing the global default policy does not affect existing sessions.

---

## Routing Resolution

The policy's `active_connection` is a zero-based index into its
`connections` list. Orchid resolves only that connection for a run. An empty
list uses the built-in local provider; an out-of-range index is invalid.

The active index is persisted in the session's agent policy snapshot.

---

# Design Principles

## Small, composable resources

Each resource represents a single architectural concept.

Large monolithic configuration files should be avoided.

---

## File-based configuration

Every reusable resource exists as an individual file.

Benefits include:

* simple sharing
* clean Git history
* minimal merge conflicts
* plugin-friendly architecture
* straightforward import/export

---

## Declarative configuration

Connections, policies and prompts are declarative definitions.

They describe behaviour but contain no mutable runtime state.

---

## Derived state over persisted decisions

Whenever possible, Orchid stores facts rather than decisions.

Runtime decisions should be derived from persisted state rather than stored independently.

This avoids duplicate sources of truth and improves determinism.

---

## Separation of concerns

Connections define how Orchid communicates with language models.

Policies define how Orchid executes work.

Prompts define assistant behaviour.

Sessions persist work performed by Orchid.

Each layer remains independent and reusable.

---

## References over duplication

Resources reference one another by name rather than embedding complete definitions.

This keeps configuration modular and allows resources to evolve independently.

---

## Extensibility

The filesystem layout intentionally mirrors Orchid's object model.

Future resource types (for example MCP server definitions, tool definitions, memory providers or agent definitions) can be introduced as additional top-level directories without requiring structural changes to the existing architecture.
