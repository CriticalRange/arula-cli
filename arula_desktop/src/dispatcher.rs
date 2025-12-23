//! Thin UI wrapper around arula_core's SessionManager
//!
//! This module provides Iced-specific subscription helpers while delegating
//! all backend logic to arula_core::SessionManager.

use arula_core::api::api::ChatMessage;
use arula_core::{SessionConfig, SessionManager, UiEvent};
use iced::Subscription;
use tokio::sync::broadcast;
use uuid::Uuid;
use std::sync::Arc;

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

    /// Returns the broadcast receiver for UI events.
    pub fn subscribe(&self) -> broadcast::Receiver<UiEvent> {
        self.manager.subscribe()
    }

    /// Creates an Iced subscription to receive UI events.
    pub fn subscription(&self) -> Subscription<UiEvent> {
        let rx = self.manager.subscribe();
        dispatcher_subscription(rx)
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

    // ==================== Conversation Starters ====================

    /// Generate contextual conversation starter suggestions
    pub fn generate_conversation_starters(&self) {
        self.manager.generate_conversation_starters();
    }
}

/// Wrapper to make the receiver hashable for run_with
#[derive(Clone)]
struct ReceiverWrapper(Arc<std::sync::Mutex<Option<broadcast::Receiver<UiEvent>>>>);

impl std::hash::Hash for ReceiverWrapper {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Use a unique id based on the Arc pointer
        std::ptr::addr_of!(*self.0).hash(state);
    }
}

/// Creates an Iced subscription to receive UI events from the dispatcher.
pub fn dispatcher_subscription(
    rx: broadcast::Receiver<UiEvent>,
) -> Subscription<UiEvent> {
    use futures::StreamExt;
    use iced::futures::channel::mpsc;
    use iced::stream;
    use tokio_stream::wrappers::BroadcastStream;
    
    let wrapper = ReceiverWrapper(Arc::new(std::sync::Mutex::new(Some(rx))));
    
    Subscription::run_with(wrapper, |wrapper: &ReceiverWrapper| {
        let rx = wrapper.0.lock().unwrap().take();
        stream::channel(100, move |mut output: mpsc::Sender<UiEvent>| async move {
            if let Some(rx) = rx {
                let mut rx_stream = BroadcastStream::new(rx);
                loop {
                    match rx_stream.next().await {
                        Some(Ok(event)) => {
                            use iced::futures::SinkExt;
                            let _ = output.send(event).await;
                        }
                        Some(Err(_)) => continue,
                        None => break,
                    }
                }
            }
        })
    })
}
