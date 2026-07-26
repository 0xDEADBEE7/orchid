# Hooks

Hooks are configured in a policy. Each event maps to an ordered list of
executable scripts:

```json
{
  "hooks": {
    "timeout": 3,
    "events": {
      "on-event": [
        {"script":"hooks/a.sh","mode":"sync"},
        {"script":"hooks/b.sh","mode":"async","timeout-seconds":10}
      ]
    }
  }
}
```

Relative scripts containing a path separator resolve from the Orchid
configuration root. Bare executable names such as `ls` are looked up through
the process `PATH`; absolute paths are used as-is. Hooks receive one
versioned JSON envelope on stdin. The envelope contains the triggering event
(`name`, `event_id`, and `event_type`) and a session snapshot equivalent to
`orchid get <ID>`. Stdout and stderr are captured for bounded lifecycle logs;
they are never interpreted as session commands.

Synchronous scripts complete before the next entry launches. Asynchronous
scripts are supervised by a detached Orchid monitor and do not block the
dispatcher. The effective timeout is `timeout-seconds`, then `hooks.timeout`,
then the implementation default.

Supported event names are `on-init`, `on-event`, `on-tool-call`,
`on-tool-result`, `on-turn-start`, `on-turn-end`, and `on-error`. Events are
persisted before their hooks run. Hook failures do not roll back persisted
events.

A hook may add context through the normal CLI, for example:

```sh
orchid send --no-run --id "$ID" "additional context"
```

This creates an ordinary user event and does not start a provider turn. Hook
dispatch has a bounded recursion guard.
