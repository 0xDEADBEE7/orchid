# orchid — User Guide

orchid is a headless CLI for running LLM sessions. Every command writes JSON to stdout, the session transcript is append-only, and execution is a background process you observe with standard tooling. Select a self-contained resource tree with `--config <DIR>`.

| Doc | Topic |
|-----|-------|
| [installation.md](installation.md) | Build and install |
| [../architecture/NEW_CONFIG.md](../architecture/NEW_CONFIG.md) | Connections, policies, prompts, and sessions |
| [sending.md](sending.md) | `orchid send` flags and workflow |
| [awaiting.md](awaiting.md) | `orchid await` orchestration and polling |
| [get.md](get.md) | `orchid get` read-only session inspection |
| [conversations.md](conversations.md) | IDs, labels, files, run lifecycle |
| [hooks.md](hooks.md) | Lifecycle hooks for automation and notification |
| [scripting.md](scripting.md) | Integration patterns, error handling, jq recipes |
| [orchestration.md](orchestration.md) | Delegating tasks and coordinating agents with `await` |
