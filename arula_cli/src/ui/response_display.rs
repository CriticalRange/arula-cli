//! Beautiful response display system for ARULA CLI
//!
//! This module provides enhanced formatting and display capabilities for AI responses,
//! including tool calls, thinking content, and markdown rendering with animations.

use crate::api::agent::ToolResult;
use crate::ui::output::OutputHandler;
use crate::utils::colors::{PRIMARY_ANSI, MISC_ANSI};
use console::style;
use std::io::{self, Write};
use std::time::{Duration, Instant};
use std::sync::mpsc::{self, Sender, Receiver};
use serde_json::Value;

/// Enhanced response display with animations and custom formatting
pub struct ResponseDisplay {
    output: OutputHandler,
    is_displaying_thinking: bool,
    accumulated_text: String,
    /// Track last tool call for updating display on completion
    last_tool_call: Option<(String, String, String)>, // (id, name, arguments)
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
            accumulated_text: String::new(),
            last_tool_call: None,
        }
    }

    /// Finalize accumulated text when stream ends
    pub fn finalize_accumulated_text(&mut self) -> io::Result<()> {
        // Just clear the accumulated text - streaming already printed it
        self.accumulated_text.clear();
        Ok(())
    }

    /// Display a tool call with pulse animation on the tool name
    pub fn display_tool_call_start(&mut self, id: &str, name: &str, arguments: &str) -> io::Result<()> {
        // Add spacing before tool call for better readability
        println!();
        
        let icon = self.get_tool_icon(name);
        let display_name = self.get_tool_display_name(name);
        let formatted_args = self.format_tool_arguments_detailed(name, arguments);

        // Store tool call info for past tense display on completion
        self.last_tool_call = Some((id.to_string(), name.to_string(), arguments.to_string()));

        // Show tool with pulse animation, then display with proper colors
        let title_part = format!("{} {}", icon, display_name);
        self.pulse_tool_line_colored(&title_part, &formatted_args, 10)?;
        
        Ok(())
    }

    /// Display a tool result - show past tense for success, error for failures
    pub fn display_tool_result(&mut self, _id: &str, tool_name: &str, result: &ToolResult) -> io::Result<()> {
        use crossterm::{cursor, execute};
        
        let icon = self.get_tool_icon(tool_name);
        
        if result.success {
            // Move up one line and overwrite with past tense
            let mut stdout = io::stdout();
            execute!(stdout, cursor::MoveToPreviousLine(1))?;
            print!("\x1b[K"); // Clear line
            
            let past_tense_name = self.get_tool_past_tense(tool_name);
            let result_summary = self.get_result_summary(tool_name, &result.data);
            
            print!("{} ", style(format!("{} {}", icon, past_tense_name)).color256(PRIMARY_ANSI).bold());
            println!("{}", style(&result_summary).color256(MISC_ANSI));
            
            // Show enhanced content for specific tools
            self.display_tool_content_preview(tool_name, &result.data)?;
            
            stdout.flush()?;
        } else {
            let display_name = self.get_tool_display_name(tool_name);
            let error_msg = self.get_error_summary(&result.data);
            println!(
                "   {} {} {}",
                style("✗").red(),
                style(&display_name).red(),
                style(&error_msg).red().dim()
            );
        }
        
        // Clear stored tool call
        self.last_tool_call = None;
        Ok(())
    }
    
    /// Display content preview for specific tools
    fn display_tool_content_preview(&self, tool_name: &str, data: &Value) -> io::Result<()> {
        // Unwrap the "Ok" wrapper if present
        let data = if let Some(ok_data) = data.get("Ok") {
            ok_data.clone()
        } else {
            data.clone()
        };
        
        match tool_name.to_lowercase().as_str() {
            "write_file" => {
                // Show written content up to 7 lines
                if let Some(content) = self.get_written_content(&data) {
                    self.display_content_lines(&content, 7, "   ")?;
                }
            }
            "execute_bash" => {
                // Show last 3 lines of output
                if let Some(stdout) = data.get("stdout").and_then(|v| v.as_str()) {
                    if !stdout.trim().is_empty() {
                        self.display_last_lines(stdout, 3, "   ")?;
                    }
                }
            }
            "edit_file" => {
                // Show diff with context
                if let Some(diff) = data.get("diff").and_then(|v| v.as_str()) {
                    self.display_diff_preview(diff, "   ")?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    
    /// Get written content from write_file result
    fn get_written_content(&self, data: &Value) -> Option<String> {
        // First try to get content_preview (new field)
        if let Some(preview) = data.get("content_preview").and_then(|v| v.as_str()) {
            return Some(preview.to_string());
        }
        None
    }
    
    /// Display content lines with a max limit
    fn display_content_lines(&self, content: &str, max_lines: usize, indent: &str) -> io::Result<()> {
        let lines: Vec<&str> = content.lines().collect();
        let total = lines.len();
        
        for (i, line) in lines.iter().take(max_lines).enumerate() {
            println!("{}{}", indent, style(line).dim());
            if i == max_lines - 1 && total > max_lines {
                println!("{}...", indent);
            }
        }
        Ok(())
    }
    
    /// Display last N lines of content
    fn display_last_lines(&self, content: &str, max_lines: usize, indent: &str) -> io::Result<()> {
        let lines: Vec<&str> = content.lines().collect();
        let total = lines.len();
        let start = total.saturating_sub(max_lines);
        
        if start > 0 {
            println!("{}...", indent);
        }
        
        for line in lines.iter().skip(start) {
            println!("{}{}", indent, style(line).dim());
        }
        Ok(())
    }
    
    /// Display diff preview with coloring
    fn display_diff_preview(&self, diff: &str, indent: &str) -> io::Result<()> {
        // The diff already has ANSI colors from the tool, just print it with indent
        for line in diff.lines().take(10) {
            // Strip existing ANSI and re-color based on +/-
            let clean_line = strip_ansi_codes(line);
            if clean_line.starts_with('+') && !clean_line.starts_with("+++") {
                println!("{}{}", indent, style(&clean_line).green());
            } else if clean_line.starts_with('-') && !clean_line.starts_with("---") {
                println!("{}{}", indent, style(&clean_line).red());
            } else {
                println!("{}{}", indent, style(&clean_line).dim());
            }
        }
        Ok(())
    }
    
    /// Get past tense version of tool name
    fn get_tool_past_tense(&self, tool_name: &str) -> String {
        match tool_name.to_lowercase().as_str() {
            "execute_bash" => "Ran".to_string(),
            "read_file" => "Read".to_string(),
            "write_file" => "Wrote".to_string(),
            "edit_file" => "Edited".to_string(),
            "list_directory" => "Listed".to_string(),
            "search_files" => "Searched".to_string(),
            "web_search" => "Searched".to_string(),
            "mcp_call" => "Called".to_string(),
            "mcp_list_tools" => "Listed".to_string(),
            "visioneer" => "Analyzed".to_string(),
            "capture_screen" => "Captured".to_string(),
            "analyze_ui" => "Analyzed".to_string(),
            _ => "Completed".to_string(),
        }
    }
    
    /// Get result summary based on tool type and result data
    fn get_result_summary(&self, tool_name: &str, data: &Value) -> String {
        // Handle case where data might be a string that needs parsing
        let data = if let Some(s) = data.as_str() {
            serde_json::from_str::<Value>(s).unwrap_or_else(|_| data.clone())
        } else {
            data.clone()
        };
        
        // Unwrap the "Ok" wrapper if present (Result<T, E> serialization)
        let data = if let Some(ok_data) = data.get("Ok") {
            ok_data.clone()
        } else {
            data
        };
        
        // Try to get path from result data
        let path_from_data = self.extract_path_from_result(&data);
        
        match tool_name.to_lowercase().as_str() {
            "edit_file" => {
                if let Some(new_content) = data.get("new_content").and_then(|v| v.as_str()) {
                    let line_count = new_content.lines().count();
                    return format!("{} lines", line_count);
                }
                if let Some(lines) = data.get("lines").and_then(|v| v.as_u64()) {
                    return format!("{} lines", lines);
                }
                if let Some(path) = path_from_data {
                    return self.shorten_path(&path);
                }
                "file".to_string()
            }
            "read_file" => {
                if let Some(lines) = data.get("lines").and_then(|v| v.as_u64()) {
                    return format!("{} lines", lines);
                }
                if let Some(content) = data.get("content").and_then(|v| v.as_str()) {
                    let line_count = content.lines().count();
                    return format!("{} lines", line_count);
                }
                if let Some(path) = path_from_data {
                    return self.shorten_path(&path);
                }
                "file".to_string()
            }
            "write_file" => {
                if let Some(bytes) = data.get("bytes_written").and_then(|v| v.as_u64()) {
                    return format!("{} bytes", bytes);
                }
                if let Some(path) = path_from_data {
                    return self.shorten_path(&path);
                }
                "file".to_string()
            }
            "list_directory" => {
                if let Some(entries) = data.get("entries").and_then(|v| v.as_array()) {
                    return format!("{} items", entries.len());
                }
                if let Some(path) = path_from_data {
                    return self.shorten_path(&path);
                }
                "directory".to_string()
            }
            "search_files" => {
                if let Some(matches) = data.get("matches").and_then(|v| v.as_array()) {
                    return format!("{} matches", matches.len());
                }
                if let Some(query) = data.get("query").and_then(|v| v.as_str()) {
                    return format!("\"{}\"", self.truncate_str(query, 30));
                }
                "files".to_string()
            }
            "web_search" => {
                if let Some(results) = data.get("results").and_then(|v| v.as_array()) {
                    return format!("{} results", results.len());
                }
                if let Some(query) = data.get("query").and_then(|v| v.as_str()) {
                    return format!("\"{}\"", self.truncate_str(query, 30));
                }
                "web".to_string()
            }
            "execute_bash" => {
                // Get command from last_tool_call if available
                let cmd_preview = self.get_command_preview();
                
                if let Some(stdout) = data.get("stdout").and_then(|v| v.as_str()) {
                    let line_count = stdout.lines().count();
                    if line_count > 0 {
                        if let Some(cmd) = &cmd_preview {
                            return format!("$ {} → {} lines", cmd, line_count);
                        }
                        return format!("{} lines output", line_count);
                    }
                    // No output case
                    if let Some(exit_code) = data.get("exit_code").and_then(|v| v.as_i64()) {
                        if exit_code == 0 {
                            if let Some(cmd) = &cmd_preview {
                                return format!("$ {}", cmd);
                            }
                            return "success (no output)".to_string();
                        }
                    }
                    if let Some(cmd) = &cmd_preview {
                        return format!("$ {} → no output", cmd);
                    }
                    return "no output".to_string();
                }
                if let Some(exit_code) = data.get("exit_code").and_then(|v| v.as_i64()) {
                    if exit_code == 0 {
                        if let Some(cmd) = &cmd_preview {
                            return format!("$ {}", cmd);
                        }
                        return "success".to_string();
                    } else {
                        if let Some(cmd) = &cmd_preview {
                            return format!("$ {} → exit {}", cmd, exit_code);
                        }
                        return format!("exit code {}", exit_code);
                    }
                }
                if let Some(cmd) = cmd_preview {
                    return format!("$ {}", cmd);
                }
                "command".to_string()
            }
            "mcp_call" => {
                if let Some(result) = data.get("result") {
                    if let Some(s) = result.as_str() {
                        let len = s.len();
                        if len > 50 {
                            return format!("{} chars", len);
                        }
                        return self.truncate_str(s, 40);
                    }
                }
                if let Some(tool) = data.get("tool_name").and_then(|v| v.as_str()) {
                    return tool.to_string();
                }
                "MCP tool".to_string()
            }
            "capture_screen" | "visioneer" | "analyze_ui" => {
                if let Some(msg) = data.get("message").and_then(|v| v.as_str()) {
                    return self.truncate_str(msg, 40);
                }
                "screen".to_string()
            }
            _ => {
                if let Some(path) = path_from_data {
                    return self.shorten_path(&path);
                }
                if let Some(msg) = data.get("message").and_then(|v| v.as_str()) {
                    return self.truncate_str(msg, 40);
                }
                "done".to_string()
            }
        }
    }
    
    /// Get command preview from stored tool call arguments (max 25 chars)
    fn get_command_preview(&self) -> Option<String> {
        if let Some((_, name, args)) = &self.last_tool_call {
            if name == "execute_bash" {
                if let Ok(parsed) = serde_json::from_str::<Value>(args) {
                    if let Some(cmd) = parsed.get("command").and_then(|v| v.as_str()) {
                        let cmd = cmd.trim();
                        if cmd.len() > 25 {
                            return Some(format!("{}...", &cmd[..22]));
                        }
                        return Some(cmd.to_string());
                    }
                }
            }
        }
        None
    }
    
    /// Extract path from result data
    fn extract_path_from_result(&self, data: &Value) -> Option<String> {
        data.get("path").and_then(|v| v.as_str()).map(|s| s.to_string())
    }
    
    /// Truncate a string to max length with ellipsis
    fn truncate_str(&self, s: &str, max_len: usize) -> String {
        if s.len() > max_len {
            format!("{}...", &s[..max_len.saturating_sub(3)])
        } else {
            s.to_string()
        }
    }
    
    /// Pulse animation for tool line with colored title and gray parameters
    fn pulse_tool_line_colored(&self, title: &str, params: &str, cycles: u32) -> io::Result<()> {
        use crossterm::{execute, style::{Color, SetForegroundColor, ResetColor}};
        
        let mut stdout = io::stdout();
        let full_text = format!("{} {}", title, params);
        
        for cycle in 0..cycles {
            let phase = (cycle as f32) / (cycles as f32) * std::f32::consts::PI * 2.0;
            let intensity = 0.3 + 0.7 * ((phase).sin() * 0.5 + 0.5);
            
            // Golden yellow pulse color (based on PRIMARY_HEX #E8C547)
            let color = Color::Rgb {
                r: (intensity * 232.0) as u8,
                g: (intensity * 197.0) as u8,
                b: (intensity * 71.0) as u8,
            };
            
            // Use \r (carriage return) for better terminal compatibility
            // \x1b[2K clears entire line, then we print from start
            execute!(stdout, SetForegroundColor(color))?;
            print!("\r\x1b[2K{}", full_text);
            stdout.flush()?;
            std::thread::sleep(Duration::from_millis(50));
        }
        
        // Final state: title in custom yellow (PRIMARY_ANSI), params in gray (MISC_ANSI)
        execute!(stdout, ResetColor)?;
        print!("\r\x1b[2K");
        print!("{}", style(title).color256(PRIMARY_ANSI).bold());
        println!(" {}", style(params).color256(MISC_ANSI));
        stdout.flush()?;
        Ok(())
    }

    /// Display thinking content - now handled minimally to avoid conversation fragmentation
    pub fn display_thinking_content(&mut self, _reasoning: &str) -> io::Result<()> {
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
            print!("\u{1F914} ");  // 🤔 Thinking face
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

    /// Display stream text - simple and direct
    pub fn display_stream_text(&mut self, text: &str) -> io::Result<()> {
        // Print directly to stdout - no filtering, no processing
        print!("{}", text);
        io::stdout().flush()?;
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

    /// Get tool-specific icon based on name (using hollow/outline symbols where available)
    fn get_tool_icon(&self, tool_name: &str) -> &'static str {
        match tool_name.to_lowercase().as_str() {
            "execute_bash" => "\u{25CB}",     // ○ White Circle (hollow shell)
            "read_file" => "\u{25CB}",       // ○ White Circle (hollow read)
            "write_file" | "edit_file" => "\u{25A1}", // □ White Square (hollow write/edit)
            "list_directory" => "\u{25C7}",  // ◇ White Diamond (hollow list)
            "search_files" => "\u{25CB}",     // ○ White Circle (hollow search)
            "web_search" => "\u{2B55}",      // ⭕ Hollow Red Circle (web/globe)
            "mcp_call" => "\u{25CA}",        // ◊ Lozenge (hollow MCP/link)
            "mcp_list_tools" => "\u{25C6}",  // ◆ Black Diamond (tools list)
            "visioneer" => "\u{25CB}",       // ○ White Circle (hollow vision)
            "capture_screen" => "\u{25CF}",   // ● Black Circle (capture)
            "analyze_ui" => "\u{25C9}",      // ◉ Fisheye (analyze)
            _ => "\u{25A1}",                 // □ White Square (default tool)
        }
    }

    /// Get human-readable tool name
    fn get_tool_display_name(&self, tool_name: &str) -> String {
        match tool_name.to_lowercase().as_str() {
            "execute_bash" => "Shell".to_string(),
            "read_file" => "Read".to_string(),
            "write_file" => "Write".to_string(),
            "edit_file" => "Edit".to_string(),
            "list_directory" => "List".to_string(),
            "search_files" => "Search".to_string(),
            "web_search" => "Web".to_string(),
            "mcp_call" => "MCP".to_string(),
            "mcp_list_tools" => "MCP Tools".to_string(),
            "visioneer" => "Vision".to_string(),
            "capture_screen" => "Screenshot".to_string(),
            "analyze_ui" => "Analyze".to_string(),
            _ => tool_name.to_string(),
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

    /// Format tool arguments compactly - just the essential info
    fn format_tool_arguments_compact(&self, tool_name: &str, arguments: &str) -> String {
        if let Ok(parsed) = serde_json::from_str::<Value>(arguments) {
            match tool_name.to_lowercase().as_str() {
                "execute_bash" => {
                    if let Some(cmd) = parsed.get("command").and_then(|v| v.as_str()) {
                        let cmd_short = if cmd.len() > 50 { format!("{}...", &cmd[..47]) } else { cmd.to_string() };
                        return cmd_short;
                    }
                }
                "read_file" => {
                    if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
                        return self.shorten_path(path);
                    }
                }
                "write_file" | "edit_file" => {
                    if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
                        return self.shorten_path(path);
                    }
                }
                "list_directory" => {
                    if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
                        return self.shorten_path(path);
                    }
                }
                "search_files" | "web_search" => {
                    if let Some(query) = parsed.get("query").and_then(|v| v.as_str()) {
                        let q_short = if query.len() > 40 { format!("{}...", &query[..37]) } else { query.to_string() };
                        return format!("\"{}\"", q_short);
                    }
                }
                "mcp_call" => {
                    if let Some(tool) = parsed.get("tool_name").and_then(|v| v.as_str()) {
                        return tool.to_string();
                    }
                }
                _ => {}
            }
        }
        // Fallback: truncate raw args
        if arguments.len() > 40 {
            format!("{}...", &arguments[..37])
        } else {
            arguments.to_string()
        }
    }

    /// Shorten a file path for display
    fn shorten_path(&self, path: &str) -> String {
        if let Some(name) = path.rsplit(['/', '\\']).next() {
            name.to_string()
        } else {
            path.to_string()
        }
    }

    /// Format tool arguments with more detail for display
    fn format_tool_arguments_detailed(&self, tool_name: &str, arguments: &str) -> String {
        if let Ok(parsed) = serde_json::from_str::<Value>(arguments) {
            match tool_name.to_lowercase().as_str() {
                "execute_bash" => {
                    if let Some(cmd) = parsed.get("command").and_then(|v| v.as_str()) {
                        let cmd_display = if cmd.len() > 80 { 
                            format!("{}...", &cmd[..77]) 
                        } else { 
                            cmd.to_string() 
                        };
                        return format!("$ {}", cmd_display);
                    }
                }
                "read_file" => {
                    if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
                        return format!("→ {}", path);
                    }
                }
                "write_file" => {
                    if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
                        let size_hint = parsed.get("content")
                            .and_then(|v| v.as_str())
                            .map(|c| format!(" ({} bytes)", c.len()))
                            .unwrap_or_default();
                        return format!("→ {}{}", path, size_hint);
                    }
                }
                "edit_file" => {
                    if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
                        return format!("→ {}", path);
                    }
                }
                "list_directory" => {
                    if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
                        return format!("→ {}", path);
                    }
                }
                "search_files" => {
                    if let Some(query) = parsed.get("query").and_then(|v| v.as_str()) {
                        let path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                        return format!("\"{}\" in {}", query, path);
                    }
                }
                "web_search" => {
                    if let Some(query) = parsed.get("query").and_then(|v| v.as_str()) {
                        return format!("\"{}\"", query);
                    }
                }
                "mcp_call" => {
                    if let Some(tool) = parsed.get("tool_name").and_then(|v| v.as_str()) {
                        let server = parsed.get("server_name").and_then(|v| v.as_str()).unwrap_or("mcp");
                        return format!("{}::{}", server, tool);
                    }
                }
                _ => {}
            }
        }
        if arguments.len() > 60 {
            arguments[..57].to_string()
        } else if arguments.is_empty() {
            String::new()
        } else {
            arguments.to_string()
        }
    }

    /// Get success summary from result data
    fn get_success_summary(&self, data: &Value) -> String {
        // Try to extract meaningful summary from result
        if let Some(output) = data.get("output").and_then(|v| v.as_str()) {
            let lines: Vec<&str> = output.lines().collect();
            if lines.len() == 1 && lines[0].len() <= 60 {
                return lines[0].to_string();
            } else if lines.len() > 1 {
                return format!("{} lines", lines.len());
            } else if !output.is_empty() {
                return format!("{} bytes", output.len());
            }
        }
        if let Some(content) = data.get("content").and_then(|v| v.as_str()) {
            let lines = content.lines().count();
            return format!("{} lines", lines);
        }
        if let Some(files) = data.get("files").and_then(|v| v.as_array()) {
            return format!("{} files", files.len());
        }
        if let Some(results) = data.get("results").and_then(|v| v.as_array()) {
            return format!("{} results", results.len());
        }
        if let Value::String(s) = data {
            if s.len() <= 40 {
                return s.clone();
            }
            return format!("{} chars", s.len());
        }
        "done".to_string()
    }

    /// Get error summary from result data
    fn get_error_summary(&self, data: &Value) -> String {
        if let Some(err) = data.get("error").and_then(|v| v.as_str()) {
            if err.len() > 60 { format!("{}...", &err[..57]) } else { err.to_string() }
        } else if let Some(msg) = data.as_str() {
            if msg.len() > 60 { format!("{}...", &msg[..57]) } else { msg.to_string() }
        } else {
            "Failed".to_string()
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

        for ch in chars.iter() {
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
                "\u{1F9E7}"  // 🧧 Brain
            ),
            LoadingType::ToolCall => (
                vec!["⟳", "⟳", "⟳", "⟳", "⟳", "⟳"],
                3,  // yellow
                "\u{26A1}"  // ⚡ Lightning
            ),
            LoadingType::ToolExecution { tool_name: _ } => (
                vec!["⏳", "⏳", "⏳", "⏳", "⏳", "⏳"],
                4,  // blue
                "\u{26A1}"  // ⚡ Lightning
            ),
            LoadingType::NetworkRequest => (
                vec!["\u{1F310}", "\u{1F310}", "\u{1F310}", "\u{1F310}", "\u{1F310}", "\u{1F310}"],
                2,  // green
                "\u{1F310}"  // 🌐 Globe
            ),
            LoadingType::Processing => (
                vec!["\u{2699}", "\u{2699}", "\u{2699}", "\u{2699}", "\u{2699}", "\u{2699}"],
                5,  // magenta
                "\u{2699}"  // ⚙ Gear
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
        _buffer_sender: Sender<String>,
    ) -> anyhow::Result<()> {
        while let Ok(response) = receiver.recv() {
            match response {
                CrossbeamResponse::StreamStart => {
                    self.display.display_separator()?;
                    // Display loading animation
                    println!("\u{1F504} Starting response generation...");  // 🔄 Refresh
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

/// Strip ANSI escape codes from a string
fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip escape sequence
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                // Skip until we hit a letter (end of escape sequence)
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}