//! Thin UI wrapper around arula_core's SessionManager
//!
//! This module provides Iced-specific subscription helpers while delegating
//! all backend logic to arula_core::SessionManager.

use arula_core::api::api::ChatMessage;
use arula_core::{SessionConfig, SessionManager, UiEvent};
use futures::StreamExt;
use iced::Subscription;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

/// UI-facing dispatcher - thin wrapper around core's SessionManager.
///
/// All backend logic lives in arula_core. This just provides Iced subscriptions.
pub struct Dispatcher {
    manager: SessionManager,
}

impl Dispatcher {
    /// Creates a new dispatcher with the given configuration.
    pub fn new(config: &arula_core::utils::config::Config) -> anyhow::Result<Self> {
        Ok(Self {
            manager: SessionManager::new(config)?,
        })
    }

    /// Updates the backend with new configuration.
    pub fn update_backend(
        &mut self,
        config: &arula_core::utils::config::Config,
    ) -> anyhow::Result<()> {
        self.manager.update_backend(config)
    }

    /// Signals that streaming should stop for the given session.
    pub fn stop_stream(&self, session_id: Uuid) {
        self.manager.stop_stream(session_id);
    }

    /// Starts a streaming session for the given prompt with conversation history.
    pub fn start_stream(
        &self,
        session_id: Uuid,
        prompt: String,
        history: Option<Vec<ChatMessage>>,
        session_config: SessionConfig,
    ) -> anyhow::Result<()> {
        self.manager
            .start_stream(session_id, prompt, history, session_config)
    }

    /// Creates an Iced subscription to receive UI events.
    pub fn subscription<Message: 'static + Send + Clone>(
        &self,
        map_fn: impl Fn(UiEvent) -> Message + Send + Clone + 'static,
    ) -> Subscription<Message> {
        let rx = self.manager.subscribe();
        Subscription::run_with_id("dispatcher-stream", {
            let rx_stream = BroadcastStream::new(rx);
            iced::futures::stream::unfold(rx_stream, |mut stream| async {
                while let Some(next) = stream.next().await {
                    match next {
                        Ok(ev) => return Some((ev, stream)),
                        Err(_) => continue,
                    }
                }
                None
            })
        })
        .map(map_fn)
    }

    // ==================== Model Fetching Delegations ====================

    pub fn fetch_openai_models(&self) {
        self.manager.fetch_openai_models();
    }

    pub fn get_cached_openai_models(&self) -> Option<Vec<String>> {
        self.manager.get_cached_openai_models()
    }

    pub fn fetch_anthropic_models(&self) {
        self.manager.fetch_anthropic_models();
    }

    pub fn get_cached_anthropic_models(&self) -> Option<Vec<String>> {
        self.manager.get_cached_anthropic_models()
    }

    pub fn fetch_ollama_models(&self) {
        self.manager.fetch_ollama_models();
    }

    pub fn get_cached_ollama_models(&self) -> Option<Vec<String>> {
        self.manager.get_cached_ollama_models()
    }

    pub fn fetch_openrouter_models(&self) {
        self.manager.fetch_openrouter_models();
    }

    pub fn get_cached_openrouter_models(&self) -> Option<Vec<String>> {
        self.manager.get_cached_openrouter_models()
    }

    pub fn fetch_zai_models(&self) {
        self.manager.fetch_zai_models();
    }

    pub fn get_cached_zai_models(&self) -> Option<Vec<String>> {
        self.manager.get_cached_zai_models()
    }
}
