use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequest {
    pub message: String,
    pub context: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    pub response: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ApiClient {
    client: Client,
    endpoint: String,
}

impl ApiClient {
    pub fn new(endpoint: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("arula-cli/1.0")
            .build()
            .expect("Failed to create HTTP client");

        Self { client, endpoint }
    }

    pub async fn send_message(&self, message: &str, context: Option<Value>) -> Result<ApiResponse> {
        let request = ApiRequest {
            message: message.to_string(),
            context,
        };

        let url = format!("{}/api/chat", self.endpoint);

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            let api_response: ApiResponse = response.json().await?;
            Ok(api_response)
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            Err(anyhow::anyhow!("API request failed: {}", error_text))
        }
    }

    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/health", self.endpoint);

        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}