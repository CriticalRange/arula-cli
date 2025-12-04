//! Web search tool
//!
//! This tool performs web searches using DuckDuckGo's API.
//! Note: The full implementation with multiple providers is in tools.rs.

use crate::api::agent::{Tool, ToolSchema, ToolSchemaBuilder};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Parameters for web search
#[derive(Debug, Deserialize)]
pub struct WebSearchParams {
    /// The search query
    pub query: String,
    /// Maximum number of results (default: 5)
    pub max_results: Option<usize>,
}

/// A single search result
#[derive(Debug, Serialize, Clone)]
pub struct WebSearchResultItem {
    /// Title of the result
    pub title: String,
    /// URL of the result
    pub url: String,
    /// Description/snippet
    pub description: String,
}

/// Result from web search
#[derive(Debug, Serialize)]
pub struct WebSearchResult {
    /// List of search results
    pub results: Vec<WebSearchResultItem>,
    /// The query that was searched
    pub query: String,
    /// Number of results returned
    pub result_count: usize,
    /// Whether the search was successful
    pub success: bool,
}

/// Web search tool using DuckDuckGo
pub struct WebSearchTool;

impl WebSearchTool {
    /// Create a new WebSearchTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    type Params = WebSearchParams;
    type Result = WebSearchResult;

    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web using DuckDuckGo. Returns titles, URLs, and descriptions."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchemaBuilder::new("web_search", "Search the web")
            .param("query", "string")
            .description("query", "The search query")
            .required("query")
            .param("max_results", "integer")
            .description("max_results", "Maximum results to return (default: 5)")
            .build()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        let WebSearchParams { query, max_results } = params;

        if query.trim().is_empty() {
            return Err("Search query cannot be empty".to_string());
        }

        let max_results = max_results.unwrap_or(5);

        // Use DuckDuckGo HTML search
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (compatible; ARULA-CLI/1.0)")
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            urlencoding::encode(&query)
        );

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Search request failed: {}", e))?;

        let html = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        // Parse results from HTML (simplified parsing)
        let mut results = Vec::new();

        // Look for result links in the HTML
        for cap in regex::Regex::new(r#"<a class="result__a" href="([^"]+)"[^>]*>([^<]+)</a>"#)
            .unwrap()
            .captures_iter(&html)
        {
            if results.len() >= max_results {
                break;
            }

            let url = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            let title = cap.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();

            // Skip empty or invalid results
            if url.is_empty() || title.is_empty() || url.starts_with("/d.js") {
                continue;
            }

            // Simple HTML entity decoding
            let decoded_title = title
                .replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&#39;", "'");

            results.push(WebSearchResultItem {
                title: decoded_title,
                url,
                description: String::new(),
            });
        }

        let result_count = results.len();

        Ok(WebSearchResult {
            results,
            query,
            result_count,
            success: true,
        })
    }
}

