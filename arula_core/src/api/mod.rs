//! API-related modules for ARULA CLI
//!
//! Contains AI client implementations, agent framework, and API communication logic.
//!
//! # Module Structure
//!
//! - `api` - Core API client for AI providers
//! - `agent` - Modern AI agent framework with type-safe tools
//! - `agent_client` - High-level agent client
//! - `models` - Unified model caching system
//! - `http_client` - Optimized HTTP client with connection pooling
//! - `streaming` - True SSE streaming with tool call accumulation

pub mod api;
pub mod agent;
pub mod agent_client;
pub mod models;
pub mod http_client;
pub mod streaming;

// Note: Types are available via their modules:
// - models::{ModelCacheManager, ModelFetcher, CachedModels}
// - http_client::{get_ai_client, get_general_client, create_streaming_client}
// - streaming::{StreamEvent, process_stream, build_streaming_request}