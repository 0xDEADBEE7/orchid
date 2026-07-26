# Hook architecture

The session append pipeline persists an event, captures the durable session
snapshot, releases the per-session write lock, and then dispatches matching
hooks. This ordering prevents a failed hook from losing an event and avoids a
deadlock when a hook invokes `orchid send --no-run`.

Policy entries use `{script, mode, timeout-seconds}`. The runner resolves
path-like scripts from the configuration root, searches bare executable names
through `PATH`, and uses absolute paths as-is. It writes the same versioned envelope to
each script, drains bounded stdout/stderr, and records lifecycle information
in the existing operational logs. Sync entries run in configured order;
async entries are handed to a detached Orchid monitor process.

The append mapper emits `on-event` for every event, `on-init` for the first
event, and tool-specific hooks for tool calls/results. Provider turns emit
`on-turn-start` and `on-turn-end`; failures emit `on-error`. A bounded internal
depth prevents hook-created messages from recursing indefinitely while keeping
those messages ordinary user events.

`send --no-run` performs only the durable user-message append and normal event
hook dispatch. It rejects writes to sessions already marked running.
