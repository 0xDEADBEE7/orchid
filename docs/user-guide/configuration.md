# Configuration

Use the self-contained resource model described in
[the new configuration architecture](../architecture/NEW_CONFIG.md).

```bash
orchid --config ./config config validate
orchid --config ./config config list
orchid --config ./config config use default
```

Connections may use `env.NAME` references. Inspection output redacts literal
API keys.
