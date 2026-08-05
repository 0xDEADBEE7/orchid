# Quickstart

This is the shortest path through one Orchid session. It assumes you already
have a configured resource tree and agent, and that `orchid` and `jq` are
available on your `PATH`.

Set the configuration directory once for this shell:

```bash
CONFIG=./config
```

## 1. Create a session

```bash
ID=$(orchid --config "$CONFIG" create --label quickstart | jq -r '.id')
echo "$ID"
```

Keep the ID: it is the stable handle for the session. The command creates an
idle session and prints JSON metadata.

## 2. Send a message

```bash
orchid --config "$CONFIG" send --id "$ID" \
  "Give me a concise overview of the most interesting thing about this project."
```

`send` returns immediately while Orchid runs the turn in the background. The
message and assistant response are appended to the session transcript.

## 3. Await completion

```bash
orchid --config "$CONFIG" await "$ID"
```

`await` observes the session until it reaches a terminal state. The default
timeout is 60 seconds; increase it for longer tasks:

```bash
orchid --config "$CONFIG" await "$ID" --timeout 300
```

A successful run normally ends in `idle`. `failed` and `cancelled` are also
terminal states and should be investigated before continuing.

## 4. Get the result

Read the last assistant response:

```bash
orchid --config "$CONFIG" get "$ID" --last-message
```

Read the complete conversation instead:

```bash
orchid --config "$CONFIG" get "$ID" --conversation
```

The session is now ready for another turn. Send a follow-up only after the
session is no longer running:

```bash
orchid --config "$CONFIG" send --id "$ID" "What should I explore next?"
```

## Complete example

```bash
CONFIG=./config
ID=$(orchid --config "$CONFIG" create --label quickstart | jq -r '.id')
orchid --config "$CONFIG" send --id "$ID" "Hello! What can you help me with?"
orchid --config "$CONFIG" await "$ID"
orchid --config "$CONFIG" get "$ID" --last-message
```
