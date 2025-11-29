//! Beautiful response display system for ARULA CLI
//!
//! This module provides enhanced formatting and display capabilities for AI responses,
//! including tool calls, thinking content, and markdown rendering with animations.

use crate::api::agent::ToolResult;
use crate::ui::output::OutputHandler;
use console::style;
use std::io::{self, Write};
use std::time::{Duration, Instant};
use std::sync::mpsc::{self, Sender, Receiver};
use serde_json::Value;

/// Enhanced response display with animations and custom formatting
pub struct ResponseDisplay {
    output: OutputHandler,
    is_displaying_thinking: bool,
}

/// Types of loading animations for different scenarios
#[derive(Debug, Clone, PartialEq)]
pub enum LoadingType {
    Thinking,
    ToolCall,
    ToolExecution { tool_name: String },
    NetworkRequest,
    Processing,
}

impl ResponseDisplay {
    pub fn new(output: OutputHandler) -> Self {
        Self {
            output,
            is_displaying_thinking: false,
        }
    }

    /// Display a tool call with beautiful formatting and animation
    pub fn display_tool_call_start(&self, _id: &str, name: &str, arguments: &str) -> io::Result<()> {
        let icon = self.get_tool_icon(name);
        let formatted_args = self.format_tool_arguments(name, arguments);

        // Display with animation effect
        print!(
            "{}{} {}",
            style("⚡").cyan(),
            icon,
            style(&format!("Running {}", formatted_args)).yellow().bold()
        );
        io::stdout().flush()?;
        println!();
        Ok(())
    }

    /// Display a tool result with success/error formatting
    pub fn display_tool_result(&mut self, _id: &str, tool_name: &str, result: &ToolResult) -> io::Result<()> {
        let status_icon = if result.success { "✅" } else { "❌" };
        let status_color = if result.success { "green" } else { "red" };
        let summary = self.summarize_tool_result(&result.data);

        self.output.print_system(&format!(
            "{} {} {}",
            style(status_icon).color256(status_color.parse::<u8>().unwrap_or(1)),
            style(&format!("{}:", tool_name)).bold(),
            style(summary).dim()
        ))
    }

    /// Display thinking content - now handled minimally to avoid conversation fragmentation
    pub fn display_thinking_content(&mut self, reasoning: &str) -> io::Result<()> {
        // For now, we don't display thinking content separately to maintain conversation flow
        // The thinking is internal reasoning that doesn't need to be shown to user
        // This prevents the conversation from feeling fragmented

        // If you want to enable thinking display in the future, uncomment:
        /*
        let processed = reasoning.trim();
        if processed.is_empty() {
            return Ok(());
        }

        if !self.is_displaying_thinking {
            print!("🤔 ");
            self.is_displaying_thinking = true;
        }

        print!("{}", style(processed).dim());
        io::stdout().flush()?;
        */

        Ok(())
    }

    /// Complete thinking content - now a no-op since we don't display thinking
    pub fn finalize_thinking_content(&mut self) -> io::Result<()> {
        // Reset flag but don't add any visual breaks
        self.is_displaying_thinking = false;
        Ok(())
    }

    /// Display stream text with markdown processing
    pub fn display_stream_text(&mut self, text: &str) -> io::Result<()> {
        // Simple markdown processing for now - can be enhanced later
        let processed_text = self.process_markdown_inline(text);
        self.output.print_ai_message(&processed_text)?;
        Ok(())
    }

    /// Display a beautiful loading animation
    pub fn display_loading_animation(&self, loading_type: LoadingType, message: &str) -> io::Result<()> {
        let (frames, color, icon) = self.get_loading_config(&loading_type);
        let mut frame_index = 0;

        // Animate for a short duration or until interrupted
        let start_time = Instant::now();
        while start_time.elapsed() < Duration::from_millis(2000) {
            let frame = frames[frame_index % frames.len()];
            // Clear the line and redraw
            self.clear_current_line()?;
            print!(
                "\r{}{} {} {}",
                icon,
                style(frame).color256(color),
                style("Processing").bold(),
                style(message).dim()
            );
            io::stdout().flush()?;
            std::thread::sleep(Duration::from_millis(150));
            frame_index += 1;
        }

        // Clear the loading line when done
        self.clear_current_line()?;
        Ok(())
    }

    /// Display multiple concurrent tool calls with scrolling
    pub fn display_concurrent_tool_calls(&mut self, tools: Vec<(String, String, String)>) -> io::Result<()> {
        if tools.is_empty() {
            return Ok(());
        }

        self.output.print_system(&format!(
            "{} Running {} tools concurrently...",
            style("⚡").cyan(),
            style(tools.len()).yellow().bold()
        ))?;

        for (index, (_id, name, args)) in tools.iter().enumerate() {
            self.output.print_system(&format!(
                "  [{}] {} {}",
                style(format!("{}", index + 1)).cyan(),
                self.get_tool_icon(name),
                style(&format!("{}: {}", name, self.format_tool_arguments(name, args))).yellow()
            ))?;
        }

        Ok(())
    }

    /// Get tool-specific icon based on name
    fn get_tool_icon(&self, tool_name: &str) -> &'static str {
        match tool_name.to_lowercase().as_str() {
            "execute_bash" => "💻",
            "read_file" => "📖",
            "write_file" | "edit_file" => "✏️",
            "list_directory" => "📁",
            "search_files" => "🔍",
            "web_search" => "🌐",
            "mcp_call" => "🔗",
            "mcp_list_tools" => "📋",
            "visioneer" => "👁️",
            "capture_screen" => "📸",
            "analyze_ui" => "🔍",
            _ => "🔧",
        }
    }

    /// Format tool arguments for display
    fn format_tool_arguments(&self, tool_name: &str, arguments: &str) -> String {
        match tool_name {
            "execute_bash" => {
                // Extract and format bash command
                if let Ok(parsed) = serde_json::from_str::<Value>(arguments) {
                    if let Some(cmd) = parsed.get("command").and_then(|v| v.as_str()) {
                        return format!("'{}'", cmd);
                    }
                }
                arguments.to_string()
            }
            "read_file" | "write_file" | "edit_file" => {
                // Extract file path
                if let Ok(parsed) = serde_json::from_str::<Value>(arguments) {
                    if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
                        return format!("'{}'", path);
                    }
                }
                arguments.to_string()
            }
            _ => arguments.to_string(),
        }
    }

    /// Summarize tool result for display
    fn summarize_tool_result(&self, result: &Value) -> String {
        match result {
            Value::String(s) => {
                if s.len() > 100 {
                    format!("{}...", &s[..97])
                } else {
                    s.clone()
                }
            }
            Value::Object(obj) => {
                if let Some(output) = obj.get("output").and_then(|v| v.as_str()) {
                    if output.len() > 100 {
                        format!("Output: {}...", &output[..97])
                    } else {
                        format!("Output: {}", output)
                    }
                } else if let Some(data) = obj.get("data").and_then(|v| v.as_str()) {
                    if data.len() > 100 {
                        format!("Data: {}...", &data[..97])
                    } else {
                        format!("Data: {}", data)
                    }
                } else {
                    format!("Result: {}", serde_json::to_string_pretty(result).unwrap_or_else(|_| "Complex data".to_string()))
                }
            }
            _ => format!("Result: {}", serde_json::to_string_pretty(result).unwrap_or_else(|_| "Complex data".to_string())),
        }
    }

    /// Process markdown inline (basic implementation)
    fn process_markdown_inline(&self, text: &str) -> String {
        text.lines()
            .map(|line| {
                // Basic markdown processing
                let mut processed = line.to_string();

                // Code blocks
                if line.trim_start().starts_with("```") {
                    return style(line).dim().to_string();
                }

                // Bold text
                while let Some(start) = processed.find("**") {
                    if let Some(relative_end) = processed[start + 2..].find("**") {
                        let end = start + 2 + relative_end;
                        // Ensure indices are valid
                        if end <= processed.len() && end + 2 <= processed.len() {
                            let before = &processed[..start];
                            let content = &processed[start + 2..end];
                            let after = &processed[end + 2..];
                            processed = format!("{}{}**{}**{}",
                                before,
                                style(content).bold(),
                                content,
                                after
                            );
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                processed
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Clear the current line for animations
    fn clear_current_line(&self) -> io::Result<()> {
        // Use ANSI escape code to clear from cursor to end of line
        // This is more precise than printing 200 spaces
        print!("\r\x1b[K");
        io::stdout().flush()
    }

    /// Print text with animation effect
    fn print_with_animation(&self, text: &str) -> io::Result<()> {
        let chars: Vec<char> = text.chars().collect();

        for (_i, ch) in chars.iter().enumerate() {
            print!("{}", ch);
            io::stdout().flush()?;
            std::thread::sleep(Duration::from_millis(10));
        }

        println!();
        Ok(())
    }

    /// Get loading animation configuration
    fn get_loading_config(&self, loading_type: &LoadingType) -> (Vec<&'static str>, u8, &'static str) {
        match loading_type {
            LoadingType::Thinking => (
                vec!["◐", "◓", "◑", "◒", "◐", "◓", "◑", "◒"],
                6,  // cyan
                "🧠"
            ),
            LoadingType::ToolCall => (
                vec!["⟳", "⟳", "⟳", "⟳", "⟳", "⟳"],
                3,  // yellow
                "⚡"
            ),
            LoadingType::ToolExecution { tool_name: _ } => (
                vec!["⏳", "⏳", "⏳", "⏳", "⏳", "⏳"],
                4,  // blue
                "⚡"
            ),
            LoadingType::NetworkRequest => (
                vec!["🌐", "🌐", "🌐", "🌐", "🌐", "🌐"],
                2,  // green
                "🌐"
            ),
            LoadingType::Processing => (
                vec!["⚙", "⚙", "⚙", "⚙", "⚙", "⚙"],
                5,  // magenta
                "⚙"
            ),
        }
    }

    /// Display a separator line
    pub fn display_separator(&mut self) -> io::Result<()> {
        self.output.print_system(&style(
            "─".repeat(60)
        ).dim().to_string())
    }
}

/// Enhanced response processor for concurrent operations
pub struct ResponseProcessor {
    display: ResponseDisplay,
}

impl ResponseProcessor {
    pub fn new(display: ResponseDisplay) -> Self {
        Self { display }
    }

    /// Process a stream of responses with enhanced display
    pub async fn process_responses_stream(
        &mut self,
        receiver: Receiver<CrossbeamResponse>,
        buffer_sender: Sender<String>,
    ) -> anyhow::Result<()> {
        while let Ok(response) = receiver.recv() {
            match response {
                CrossbeamResponse::StreamStart => {
                    self.display.display_separator()?;
                    // Display loading animation
                    println!("🔄 Starting response generation...");
                    self.display.display_separator()?;
                }
                CrossbeamResponse::StreamText(text) => {
                    self.display.display_stream_text(&text)?;
                }
                CrossbeamResponse::ThinkingContent(reasoning) => {
                    self.display.display_thinking_content(&reasoning)?;
                }
                CrossbeamResponse::ToolCall { id, name, arguments } => {
                    self.display.display_tool_call_start(&id, &name, &arguments)?;
                }
                CrossbeamResponse::ToolResult { id, tool_name, result } => {
                    self.display.display_tool_result(&id, &tool_name, &result)?;
                }
                CrossbeamResponse::StreamEnd => {
                    self.display.display_separator()?;
                }
            }
        }
        Ok(())
    }
}

/// Enhanced AI response types for crossbeam communication
#[derive(Debug, Clone)]
pub enum CrossbeamResponse {
    StreamStart,
    StreamText(String),
    ThinkingContent(String),
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    ToolResult {
        id: String,
        tool_name: String,
        result: ToolResult,
    },
    StreamEnd,
}

/// Enhanced input manager for persistent input during AI processing
pub struct InputManager {
    buffer: String,
    is_enabled: bool,
    response_sender: Sender<CrossbeamResponse>,
}

impl InputManager {
    pub fn new() -> (Self, Sender<CrossbeamResponse>, Receiver<CrossbeamResponse>) {
        let (tx, rx) = mpsc::channel();
        (
            Self {
                buffer: String::new(),
                is_enabled: false,
                response_sender: tx.clone(),
            },
            tx,
            rx,
        )
    }

    /// Enable persistent input during AI processing
    pub fn enable_persistent_input(&mut self) {
        self.is_enabled = true;
    }

    /// Disable persistent input during AI processing
    pub fn disable_persistent_input(&mut self) {
        self.is_enabled = false;
    }

    /// Get current input buffer
    pub fn get_input(&self) -> String {
        self.buffer.clone()
    }

    /// Add character to input buffer
    pub fn add_char(&mut self, ch: char) {
        self.buffer.push(ch);
    }

    /// Remove last character from input buffer
    pub fn backspace(&mut self) {
        self.buffer.pop();
    }

    /// Clear input buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Take buffered input (for sending to AI)
    pub fn take_input(&mut self) -> String {
        let input = self.buffer.clone();
        self.buffer.clear();
        input
    }

    
    /// Send response to display system
    pub fn send_response(&self, response: CrossbeamResponse) -> anyhow::Result<()> {
        self.response_sender.send(response).map_err(|e| anyhow::anyhow!("Failed to send response: {}", e))
    }
}