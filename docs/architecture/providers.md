# Providers

## Rust modules

The provider contract is defined in `src/provider/mod.rs`:

```rust
pub trait Provider: Send + Sync {
    fn send(&self, system: String, messages: Vec<Message>)
        -> Result<Response, ProviderError>;
    fn send_streaming(&self, system: String, messages: Vec<Message>)
        -> Result<Box<dyn Iterator<Item = Result<StreamEvent, ProviderError>>>, ProviderError>;
}
```

`Response` contains optional message, reasoning, tool calls, token usage, and
model fields. `StreamEvent` represents text, reasoning, tool-call deltas, and
stream completion. The execution loop depends on this trait, not on a concrete
provider client.

## Client modules

- `src/client/base.rs` — the `Client` trait and shared request/event types.
- `src/client/openai_chat.rs` — OpenAI-compatible chat completions and SSE parsing.
- `src/client/openai_codex/` — OpenAI Codex Responses client and OAuth tokens.
- `src/client/echo.rs` — returns the latest message without a network request.
- `src/client/factory.rs` — selects a client for a resolved connection.

## Client selection

`src/client/mod.rs` creates a client from the single connection selected by the
policy's `active_connection` index. Supported interfaces are `echo`,
`openai` (and its alias `local`), and `codex` (alias `openai-codex`); an
`openai` connection with Codex OAuth credentials also selects the Codex client.
Anthropic models are reachable through OpenAI-compatible endpoints. The `echo` client returns the
latest user message without making a network request. See
[NEW_CONFIG.md](NEW_CONFIG.md) for resource configuration and
[execution.md](execution.md) for loop behavior.
