#![allow(dead_code)]
#![allow(private_interfaces)]

pub mod api;
pub mod app;
pub mod prelude;
pub mod tools;
pub mod utils;

use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

pub use api::agent::{ContentBlock, ToolRegistry};
pub use api::api::Usage;
pub use app::App;
pub use prelude::*;
pub use tools::*;
pub use utils::*;

/// High-level streaming events exposed to consumers (CLI/desktop).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamEvent {
    Start { model: String },
    Text { text: String },
    Reasoning { text: String },
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    ToolResult {
        tool_call_id: String,
        result: api::agent::ToolResult,
    },
    Finished,
    Error(String),
}

/// Session configuration for streaming calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub system_prompt: String,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

/// Backend trait for pluggable providers.
pub trait Backend: Send + Sync + Clone + 'static {
    fn stream_session(
        &self,
        prompt: String,
        history: Option<Vec<api::api::ChatMessage>>,
        config: SessionConfig,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>>;
}

/// Session runner wrapping a backend implementation.
#[derive(Clone)]
pub struct SessionRunner<B: Backend> {
    backend: B,
}

impl<B: Backend> SessionRunner<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    pub fn stream_session(
        &self,
        prompt: String,
        history: Option<Vec<api::api::ChatMessage>>,
        config: SessionConfig,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>> {
        self.backend.stream_session(prompt, history, config)
    }
}

/// Agent-backed implementation using the existing AgentClient.
#[derive(Clone)]
pub struct AgentBackend {
    client: api::agent_client::AgentClient,
}

impl AgentBackend {
    pub fn new(config: &utils::config::Config, system_prompt: String) -> anyhow::Result<Self> {
        let agent_options = api::agent::AgentOptionsBuilder::new()
            .system_prompt(&system_prompt)
            .model(&config.get_model())
            .auto_execute_tools(true)
            .max_tool_iterations(1000)
            .debug(utils::debug::is_debug_enabled())
            .build();

        let tool_registry = tools::tools::create_basic_tool_registry();

        let client = api::agent_client::AgentClient::new_with_registry(
            config.active_provider.clone(),
            config.get_api_url(),
            config.get_api_key(),
            config.get_model(),
            agent_options,
            config,
            tool_registry,
        );

        Ok(Self { client })
    }
}

impl Backend for AgentBackend {
    fn stream_session(
        &self,
        prompt: String,
        history: Option<Vec<api::api::ChatMessage>>,
        config: SessionConfig,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>> {
        let client = self.client.clone();
        let model = config.model.clone();
        let prompt = prompt.clone();
        let stream = async_stream::stream! {
            use futures::StreamExt;
            yield StreamEvent::Start { model: model.clone() };
            match client.query_streaming(&prompt, history).await {
                Ok(mut s) => {
                    while let Some(block) = s.next().await {
                        let ev = match block {
                            ContentBlock::Text { text } => StreamEvent::Text { text },
                            ContentBlock::Reasoning { reasoning } => StreamEvent::Reasoning { text: reasoning },
                            ContentBlock::ToolCall { id, name, arguments } => StreamEvent::ToolCall { id, name, arguments },
                            ContentBlock::ToolResult { tool_call_id, result } => StreamEvent::ToolResult { tool_call_id, result },
                            ContentBlock::Error { error } => StreamEvent::Error(error),
                        };
                        yield ev;
                    }
                    yield StreamEvent::Finished;
                }
                Err(err) => {
                    yield StreamEvent::Error(err.to_string());
                }
            }
        };
        Ok(Box::pin(stream) as Pin<Box<dyn Stream<Item = StreamEvent> + Send>>)
    }
}
