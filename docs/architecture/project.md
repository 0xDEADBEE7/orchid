# Project architecture

Orchid is a headless CLI whose configuration is a self-contained resource tree
selected with `--config <DIR>`. The active architecture consists of
Connections, Policies, Prompts, and Sessions. See [NEW_CONFIG.md](NEW_CONFIG.md)
for the storage contract and [execution.md](execution.md) for runtime flow.

The implementation does not use profiles, personas, global conversation stores,
or migration adapters.
