//! Conversation management for saving and loading chat histories.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

use crate::session_manager::UiEvent;

/// Metadata for a saved conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMetadata {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
    pub model: String,
}

impl ConversationMetadata {
    /// Creates metadata for a conversation from its messages.
    pub fn from_events(id: Uuid, events: &[UiEvent], model: String) -> Self {
        let mut message_count = 0;
        let mut created_at = Utc::now();
        let mut updated_at = Utc::now();
        let mut title = "New Conversation".to_string();

        for event in events {
            match event {
                UiEvent::UserMessage { timestamp, .. } | UiEvent::AiMessage { timestamp, .. } => {
                    message_count += 1;
                    if let Ok(ts) = DateTime::parse_from_rfc3339(timestamp) {
                        let dt = ts.with_timezone(&Utc);
                        if message_count == 1 {
                            created_at = dt;
                        }
                        updated_at = dt;
                    }
                    
                    // Use first user message as title (truncated) if no title was generated
                    if title == "New Conversation" && matches!(event, UiEvent::UserMessage { content: _, .. }) {
                        if let UiEvent::UserMessage { content: user_content, .. } = event {
                            title = if user_content.len() > 50 {
                                format!("{}...", &user_content[..50].trim())
                            } else {
                                user_content.clone()
                            };
                        }
                    }
                }
                UiEvent::ConversationTitle(generated_title) => {
                    // Use the AI-generated title
                    title = generated_title.clone();
                }
                _ => {}
            }
        }

        Self {
            id,
            title,
            created_at,
            updated_at,
            message_count,
            model,
        }
    }

    /// Returns a human-readable relative time string for when the conversation was last updated.
    pub fn relative_time(&self) -> String {
        crate::utils::time::relative_time(self.updated_at)
    }
}

/// Complete saved conversation including metadata and events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedConversation {
    pub metadata: ConversationMetadata,
    pub events: Vec<UiEvent>,
}

/// Manager for conversation storage and retrieval.
#[derive(Debug)]
pub struct ConversationManager {
    storage_dir: PathBuf,
}

impl ConversationManager {
    /// Creates a new conversation manager.
    pub fn new() -> Result<Self> {
        let storage_dir = dirs::home_dir()
            .context("Could not find home directory")?
            .join(".arula")
            .join("conversations");

        // Create storage directory if it doesn't exist
        fs::create_dir_all(&storage_dir)
            .with_context(|| format!("Failed to create conversations directory: {:?}", storage_dir))?;

        Ok(Self { storage_dir })
    }

    /// Saves a conversation with the given ID and events.
    pub fn save_conversation(&self, id: Uuid, events: &[UiEvent], model: String) -> Result<()> {
        if events.is_empty() {
            return Ok(()); // Don't save empty conversations
        }

        let metadata = ConversationMetadata::from_events(id, events, model);
        let conversation = SavedConversation { metadata, events: events.to_vec() };

        let file_path = self.storage_dir.join(format!("{}.json", id));
        let json = serde_json::to_string_pretty(&conversation)
            .context("Failed to serialize conversation")?;

        fs::write(file_path, json)
            .context("Failed to write conversation file")?;

        Ok(())
    }

    /// Loads a conversation by ID.
    pub fn load_conversation(&self, id: Uuid) -> Result<SavedConversation> {
        let file_path = self.storage_dir.join(format!("{}.json", id));
        
        if !file_path.exists() {
            return Err(anyhow::anyhow!("Conversation not found: {}", id));
        }

        let content = fs::read_to_string(&file_path)
            .context("Failed to read conversation file")?;
        
        let conversation: SavedConversation = serde_json::from_str(&content)
            .context("Failed to deserialize conversation")?;

        Ok(conversation)
    }

    /// Lists all saved conversations with their metadata.
    pub fn list_conversations(&self) -> Result<Vec<ConversationMetadata>> {
        let mut conversations = Vec::new();

        if !self.storage_dir.exists() {
            return Ok(conversations);
        }

        for entry in fs::read_dir(&self.storage_dir)
            .context("Failed to read conversations directory")? 
        {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = fs::read_to_string(&path)
                    .context("Failed to read conversation file")?;
                
                let conversation: SavedConversation = serde_json::from_str(&content)
                    .context("Failed to deserialize conversation")?;
                
                conversations.push(conversation.metadata);
            }
        }

        // Sort by most recently updated first
        conversations.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(conversations)
    }

    /// Deletes a conversation by ID.
    pub fn delete_conversation(&self, id: Uuid) -> Result<()> {
        let file_path = self.storage_dir.join(format!("{}.json", id));
        
        if file_path.exists() {
            fs::remove_file(&file_path)
                .context("Failed to delete conversation file")?;
        }

        Ok(())
    }

    /// Updates the metadata of a conversation (e.g., title change).
    pub fn update_conversation_metadata(&self, metadata: &ConversationMetadata) -> Result<()> {
        let mut conversation = self.load_conversation(metadata.id)?;
        conversation.metadata = metadata.clone();

        let file_path = self.storage_dir.join(format!("{}.json", metadata.id));
        let json = serde_json::to_string_pretty(&conversation)
            .context("Failed to serialize conversation")?;

        fs::write(file_path, json)
            .context("Failed to write conversation file")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_manager::UiEvent;

    #[test]
    fn test_conversation_metadata_creation() {
        let id = Uuid::new_v4();
        let events = vec![
            UiEvent::UserMessage {
                content: "Hello, how are you?".to_string(),
                timestamp: Utc::now().to_rfc3339(),
            },
            UiEvent::AiMessage {
                content: "I'm doing well, thank you!".to_string(),
                timestamp: Utc::now().to_rfc3339(),
            },
        ];

        let metadata = ConversationMetadata::from_events(id, &events, "gpt-4".to_string());
        
        assert_eq!(metadata.id, id);
        assert_eq!(metadata.message_count, 2);
        assert_eq!(metadata.model, "gpt-4");
        assert_eq!(metadata.title, "Hello, how are you?");
    }

    #[test]
    fn test_title_truncation() {
        let id = Uuid::new_v4();
        let long_content = "This is a very long message that should be truncated when used as a title for the conversation because it exceeds the maximum length limit that we have set for conversation titles which is fifty characters";
        let events = vec![
            UiEvent::UserMessage {
                content: long_content.to_string(),
                timestamp: Utc::now().to_rfc3339(),
            },
        ];

        let metadata = ConversationMetadata::from_events(id, &events, "gpt-4".to_string());
        
        assert!(metadata.title.len() <= 53); // 50 chars + "..."
        assert!(metadata.title.ends_with("..."));
    }
}