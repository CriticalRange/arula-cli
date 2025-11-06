use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Local};
use crate::chat::ChatMessage;

const ARULA_DIR: &str = ".arula";
const MEMORY_FILE: &str = "ARULA.md";
const CONVERSATIONS_DIR: &str = "conversations";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub messages: Vec<ChatMessage>,
}

impl Conversation {
    pub fn new(title: String) -> Self {
        let now = Local::now();
        let id = format!("conv_{}", now.format("%Y%m%d_%H%M%S"));

        Self {
            id,
            title,
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
        }
    }

    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
        self.updated_at = Local::now();
    }
}

pub struct ConversationManager {
    arula_dir: PathBuf,
    memory_file: PathBuf,
    conversations_dir: PathBuf,
    current_conversation: Option<Conversation>,
}

impl ConversationManager {
    pub fn new() -> Result<Self> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;

        let arula_dir = home_dir.join(ARULA_DIR);
        let memory_file = arula_dir.join(MEMORY_FILE);
        let conversations_dir = arula_dir.join(CONVERSATIONS_DIR);

        // Create directories if they don't exist
        fs::create_dir_all(&arula_dir)?;
        fs::create_dir_all(&conversations_dir)?;

        // Create ARULA.md if it doesn't exist
        if !memory_file.exists() {
            let default_memory = r#"# ARULA - Persistent Memory

## About Me
ARULA is an Autonomous AI CLI interface designed to help with coding, Git operations, and shell commands.

## User Preferences
- (Add user preferences here as we learn them)

## Important Notes
- (Add important information to remember across sessions)

## Projects
- (Track projects and their context)
"#;
            fs::write(&memory_file, default_memory)?;
        }

        Ok(Self {
            arula_dir,
            memory_file,
            conversations_dir,
            current_conversation: None,
        })
    }

    pub fn get_memory(&self) -> Result<String> {
        Ok(fs::read_to_string(&self.memory_file)?)
    }

    pub fn update_memory(&self, content: &str) -> Result<()> {
        fs::write(&self.memory_file, content)?;
        Ok(())
    }

    pub fn start_new_conversation(&mut self, title: String) -> Result<()> {
        let conversation = Conversation::new(title);
        self.current_conversation = Some(conversation);
        Ok(())
    }

    pub fn get_current_conversation(&self) -> Option<&Conversation> {
        self.current_conversation.as_ref()
    }

    pub fn get_current_conversation_mut(&mut self) -> Option<&mut Conversation> {
        self.current_conversation.as_mut()
    }

    pub fn add_message_to_current(&mut self, message: ChatMessage) -> Result<()> {
        if let Some(conv) = &mut self.current_conversation {
            conv.add_message(message);
            Ok(())
        } else {
            Err(anyhow::anyhow!("No active conversation"))
        }
    }

    pub fn save_current_conversation(&self) -> Result<()> {
        if let Some(conv) = &self.current_conversation {
            let file_path = self.conversations_dir.join(format!("{}.json", conv.id));
            let json = serde_json::to_string_pretty(conv)?;
            fs::write(file_path, json)?;
        }
        Ok(())
    }

    pub fn load_conversation(&mut self, conversation_id: &str) -> Result<()> {
        let file_path = self.conversations_dir.join(format!("{}.json", conversation_id));
        let json = fs::read_to_string(file_path)?;
        let conversation: Conversation = serde_json::from_str(&json)?;
        self.current_conversation = Some(conversation);
        Ok(())
    }

    pub fn list_conversations(&self) -> Result<Vec<String>> {
        let mut conversations = Vec::new();

        for entry in fs::read_dir(&self.conversations_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(file_name) = path.file_stem().and_then(|s| s.to_str()) {
                    conversations.push(file_name.to_string());
                }
            }
        }

        conversations.sort();
        conversations.reverse(); // Most recent first

        Ok(conversations)
    }

    pub fn get_conversation_info(&self, conversation_id: &str) -> Result<Conversation> {
        let file_path = self.conversations_dir.join(format!("{}.json", conversation_id));
        let json = fs::read_to_string(file_path)?;
        let conversation: Conversation = serde_json::from_str(&json)?;
        Ok(conversation)
    }

    pub fn delete_conversation(&self, conversation_id: &str) -> Result<()> {
        let file_path = self.conversations_dir.join(format!("{}.json", conversation_id));
        fs::remove_file(file_path)?;
        Ok(())
    }

    pub fn get_arula_dir(&self) -> &Path {
        &self.arula_dir
    }
}

impl Default for ConversationManager {
    fn default() -> Self {
        Self::new().expect("Failed to initialize ConversationManager")
    }
}
