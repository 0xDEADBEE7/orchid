use super::{Client, ClientError, ClientEvent, ClientRequest};

pub struct EchoClient;

impl Client for EchoClient {
    fn stream(&self, request: ClientRequest) -> Result<Vec<ClientEvent>, ClientError> {
        let message = request
            .messages
            .last()
            .map(|message| message.content.clone())
            .unwrap_or_default();
        Ok(vec![ClientEvent::TextDelta(message), ClientEvent::Done])
    }
}
