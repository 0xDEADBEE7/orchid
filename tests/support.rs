use orchid::provider::{Provider, ProviderError, Response, StreamEvent};
use orchid::types::Message;
use std::path::PathBuf;

pub struct TestEnv {
    prev: Option<String>,
    temp: tempfile::TempDir,
}

impl TestEnv {
    pub fn new() -> Self {
        let temp = tempfile::TempDir::new().unwrap();
        let dir = temp.path().to_path_buf();
        let prev = std::env::var("ORCHID_DIR").ok();
        std::env::set_var("ORCHID_DIR", dir.to_string_lossy().to_string());
        Self { prev, temp }
    }

    pub fn dir(&self) -> PathBuf {
        self.temp.path().to_path_buf()
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        match &self.prev {
            Some(v) => std::env::set_var("ORCHID_DIR", v.clone()),
            None => std::env::remove_var("ORCHID_DIR"),
        }
    }
}

pub struct MockProvider {
    pub responses: Vec<Response>,
    pub errors: Vec<ProviderError>,
    pub call_count: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

impl MockProvider {
    pub fn new() -> Self {
        Self {
            responses: Vec::new(),
            errors: Vec::new(),
            call_count: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0)),
        }
    }

    pub fn with_response(mut self, response: Response) -> Self {
        self.responses.push(response);
        self
    }

    pub fn with_error(mut self, error: ProviderError) -> Self {
        self.errors.push(error);
        self
    }

    pub fn calls(&self) -> u32 {
        self.call_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Provider for MockProvider {
    fn send(&self, _system: String, _messages: Vec<Message>) -> Result<Response, ProviderError> {
        let count = self.call_count.load(std::sync::atomic::Ordering::SeqCst);
        self.call_count
            .store(count + 1, std::sync::atomic::Ordering::SeqCst);
        if let Some(error) = self.errors.last() {
            return Err(error.clone());
        }
        self.responses.last().cloned().ok_or_else(|| {
            ProviderError::InvalidResponse("no mock response configured".to_string())
        })
    }

    fn send_streaming(
        &self,
        _system: String,
        _messages: Vec<Message>,
    ) -> Result<Box<dyn Iterator<Item = Result<StreamEvent, ProviderError>>>, ProviderError> {
        Ok(Box::new(std::iter::empty()))
    }
}
