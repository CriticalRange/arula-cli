//! Question tool for AI to ask clarifying questions
//!
//! This tool allows the AI to ask one or more questions to the user for clarification.
//! Uses tokio oneshot channels to block until user responds.

use crate::api::agent::{Tool, ToolSchema, ToolSchemaBuilder};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

// Global question handler for managing pending questions
lazy_static::lazy_static! {
    pub static ref QUESTION_HANDLER: QuestionHandler = QuestionHandler::new();
}

/// A single question with optional answer choices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    /// Unique identifier for this question
    pub id: String,
    /// The question text
    pub question: String,
    /// Optional list of suggested answers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
}

/// A pending question waiting for user response
pub struct PendingQuestionEntry {
    pub questions: Vec<Question>,
    pub response_tx: oneshot::Sender<Vec<Answer>>,
}

/// An answer to a question
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answer {
    /// The question ID being answered
    pub id: String,
    /// The user's answer
    pub answer: String,
    /// Whether user typed custom or selected option
    #[serde(rename = "type")]
    pub answer_type: String, // "selected" or "custom"
}

/// Handler for managing pending questions
pub struct QuestionHandler {
    pending: Arc<Mutex<HashMap<String, PendingQuestionEntry>>>,
}

impl QuestionHandler {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register new questions and return a receiver for the responses
    pub fn ask(&self, batch_id: String, questions: Vec<Question>) -> oneshot::Receiver<Vec<Answer>> {
        let (tx, rx) = oneshot::channel();
        
        // Store the pending questions
        {
            let mut pending = self.pending.lock().unwrap();
            pending.insert(batch_id.clone(), PendingQuestionEntry {
                questions,
                response_tx: tx,
            });
        }
        
        rx
    }

    /// Answer all pending questions in a batch
    pub fn answer(&self, batch_id: &str, answers: Vec<Answer>) -> Result<(), String> {
        let pending_entry = {
            let mut pending = self.pending.lock().unwrap();
            pending.remove(batch_id)
        };
        
        match pending_entry {
            Some(entry) => {
                entry.response_tx.send(answers).map_err(|_| "Failed to send answers".to_string())
            }
            None => Err(format!("No pending questions with batch id: {}", batch_id))
        }
    }

    /// Get list of pending batch IDs
    pub fn get_pending_ids(&self) -> Vec<String> {
        let pending = self.pending.lock().unwrap();
        pending.keys().cloned().collect()
    }

    /// Check if there are pending questions
    pub fn has_pending(&self) -> bool {
        let pending = self.pending.lock().unwrap();
        !pending.is_empty()
    }

    /// Get details of all pending question batches (batch_id, questions)
    pub fn get_pending_questions(&self) -> Vec<(String, Vec<Question>)> {
        let pending = self.pending.lock().unwrap();
        pending.iter().map(|(id, entry)| {
            (id.clone(), entry.questions.clone())
        }).collect()
    }
}

impl Default for QuestionHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Parameters for asking questions - supports multiple questions
#[derive(Debug, Deserialize)]
pub struct QuestionParams {
    /// Array of questions to ask the user
    pub questions: Vec<QuestionInput>,
}

/// Input format for a single question
#[derive(Debug, Deserialize)]
pub struct QuestionInput {
    /// The question text
    pub question: String,
    /// Optional list of suggested answers
    #[serde(default)]
    pub options: Option<Vec<String>>,
}

/// Result from questions - returns all answers in JSON format
#[derive(Debug, Serialize)]
pub struct QuestionResult {
    /// All answers from the user
    pub answers: Vec<Answer>,
    /// Success status
    pub success: bool,
}

/// Question tool for clarification
///
/// This tool is used when the AI needs to ask the user for
/// more information before proceeding with a task.
/// It BLOCKS until the user provides answers to all questions.
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
        "Ask the user one or more clarifying questions and wait for their responses. Returns answers in JSON format."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchemaBuilder::new("ask_question", "Ask the user one or more clarifying questions")
            .param("questions", "array")
            .description("questions", "Array of question objects. Each object has 'question' (string, required) and 'options' (array of strings, optional)")
            .required("questions")
            .build()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        let QuestionParams { questions } = params;

        if questions.is_empty() {
            return Err("At least one question is required".to_string());
        }

        // Generate unique batch ID
        let batch_id = uuid::Uuid::new_v4().to_string();
        
        // Convert inputs to Questions with generated IDs
        let question_list: Vec<Question> = questions.into_iter().enumerate().map(|(i, q)| {
            Question {
                id: format!("q{}", i + 1),
                question: q.question,
                options: q.options,
            }
        }).collect();
        
        eprintln!("🔔 ask_question tool: Asking {} questions", question_list.len());
        for q in &question_list {
            eprintln!("   - [{}] '{}' options: {:?}", q.id, q.question, q.options);
        }
        
        // Register questions and get receiver
        let rx = QUESTION_HANDLER.ask(batch_id.clone(), question_list);
        
        eprintln!("🔔 ask_question tool: Waiting for user responses...");
        
        // BLOCK until user responds to all questions
        match rx.await {
            Ok(answers) => {
                eprintln!("🔔 ask_question tool: Received {} answers", answers.len());
                Ok(QuestionResult {
                    answers,
                    success: true,
                })
            }
            Err(_) => {
                Err("Questions were cancelled or channel closed".to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_question_tool_creation() {
        let tool = QuestionTool::new();
        assert_eq!(tool.name(), "ask_question");
    }
}
