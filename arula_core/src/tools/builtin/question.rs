//! Question tool for AI to ask clarifying questions
//!
//! This tool allows the AI to ask questions to the user for clarification.

use crate::api::agent::{Tool, ToolSchema, ToolSchemaBuilder};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Parameters for asking a question
#[derive(Debug, Deserialize)]
pub struct QuestionParams {
    /// The question to ask the user
    pub question: String,
    /// Optional list of suggested answers
    pub options: Option<Vec<String>>,
}

/// Result from question (placeholder for user response)
#[derive(Debug, Serialize)]
pub struct QuestionResult {
    /// The question that was asked
    pub question: String,
    /// Indicator that user input is needed
    pub awaiting_response: bool,
    /// Success status
    pub success: bool,
}

/// Question tool for clarification
///
/// This tool is used when the AI needs to ask the user for
/// more information before proceeding with a task.
pub struct QuestionTool;

impl QuestionTool {
    /// Create a new QuestionTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for QuestionTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for QuestionTool {
    type Params = QuestionParams;
    type Result = QuestionResult;

    fn name(&self) -> &str {
        "ask_question"
    }

    fn description(&self) -> &str {
        "Ask the user a clarifying question when more information is needed."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchemaBuilder::new("ask_question", "Ask the user a clarifying question")
            .param("question", "string")
            .description("question", "The question to ask the user")
            .required("question")
            .param("options", "array")
            .description("options", "Optional list of suggested answers")
            .build()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        let QuestionParams { question, options: _ } = params;

        if question.trim().is_empty() {
            return Err("Question cannot be empty".to_string());
        }

        // The actual user interaction is handled by the UI layer
        // This tool just signals that a question needs to be asked
        Ok(QuestionResult {
            question,
            awaiting_response: true,
            success: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ask_question() {
        let tool = QuestionTool::new();
        let result = tool.execute(QuestionParams {
            question: "What is your preferred language?".to_string(),
            options: Some(vec!["Rust".to_string(), "Python".to_string()]),
        }).await.unwrap();

        assert!(result.success);
        assert!(result.awaiting_response);
        assert_eq!(result.question, "What is your preferred language?");
    }

    #[tokio::test]
    async fn test_empty_question_error() {
        let tool = QuestionTool::new();
        let result = tool.execute(QuestionParams {
            question: "".to_string(),
            options: None,
        }).await;

        assert!(result.is_err());
    }
}

