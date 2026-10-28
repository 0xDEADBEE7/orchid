//! Provides the mod functionality.
pub mod base;
mod echo;
mod endpoint;
pub mod factory;
pub mod openai_chat;
pub mod openai_codex;

pub use base::{
    Client, ClientError, ClientErrorKind, ClientEvent, ClientRequest, Message, ToolDefinition,
};
pub use echo::EchoClient;
pub use factory::client_for;
