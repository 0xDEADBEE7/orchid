pub mod base;
mod echo;
pub mod factory;
pub mod openai_chat;
pub mod openai_codex;
pub mod transport;

pub use base::{
    Client, ClientError, ClientErrorKind, ClientEvent, ClientRequest, Message, ToolDefinition,
};
pub use echo::EchoClient;
pub use factory::client_for;
