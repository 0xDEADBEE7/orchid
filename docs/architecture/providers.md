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

- `src/client/base.rs` — shared HTTP client, response handling, and retry policy.
- `src/client/resolve.rs` — connection and credential/environment resolution.
- `src/client/anthropic/` — Anthropic request mapping and SSE handling.
- `src/client/openai/` — OpenAI-compatible request mapping and SSE handling.
- `src/client/codex.rs` and `src/client/codex_auth.rs` — OpenAI Codex OAuth
  transport and token handling.
- `src/client/sse/` — shared streaming parser and tool-call accumulation.

Provider-specific wire types and mapping stay in each provider module. The
shared `BaseClient` handles common HTTP concerns; clients implement the
`Provider` trait directly.

## Client selection

`src/client/mod.rs` creates an `Arc<dyn Provider>` from the ordered Connection
resources resolved from a Policy. Supported interfaces are `anthropic` and
`openai`; an `openai` connection using `openai_codex_oauth` selects the Codex
client. Candidates are tried in order until one can be created. See
[NEW_CONFIG.md](NEW_CONFIG.md) for resource configuration and
[execution.md](execution.md) for loop behavior.
