//! Keeps Codex gateway compatibility headers consistent across HTTP clients.

/// Codex protocol version verified against the Astra gateway.
pub const VERSION: &str = "0.153.4";
/// Identifies Orchid while retaining the gateway's Codex client prefix.
pub const USER_AGENT: &str = "codex_cli_rs/0.153.4 (orchid)";
