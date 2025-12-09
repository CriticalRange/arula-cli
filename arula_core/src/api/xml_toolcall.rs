//! XML to OpenAI ToolCall Converter
//!
//! A lightweight, tolerant parser for extracting function calls from XML-formatted
//! reasoning content (primarily for GLM-4.6 and similar models that output tool calls
//! as XML rather than the standard OpenAI function calling format).
//!
//! Design principles:
//! - Pull-driven parsing (no DOM)
//! - Tolerant of malformed XML
//! - Always takes the LAST tool_call seen
//! - Produces stable OpenAI-style output

use quick_xml::events::Event;
use quick_xml::Reader;
use serde_json::json;

/// State machine for XML tool call extraction
#[derive(Debug)]
enum ParseState {
    Idle,
    InsideToolCall(String),             // name
    InsideArguments(String),            // name
    ArgumentsCollected(String, String), // name, arguments_text
}

/// Extract tool calls from XML-formatted reasoning content.
///
/// Supports two XML formats:
/// 1. Standard format:
/// ```xml
/// <tool_call name="list_directory">
///   <arguments>{"path": "."}</arguments>
/// </tool_call>
/// ```
///
/// 2. GLM-4.6 format:
/// ```xml
/// <tool_call>execute_bash
/// <arg_key>command</arg_key>
/// <arg_value>echo "test"</arg_value>
/// </tool_call>
/// ```
///
/// Returns the last valid tool call found, or None if none exists.
pub fn extract_tool_call_from_xml(xml_text: &str) -> Option<serde_json::Value> {
    // Try GLM-4.6 format first (more common)
    if let Some(result) = extract_glm46_format(xml_text) {
        return Some(result);
    }

    // Fall back to standard format
    extract_standard_format(xml_text)
}

/// Extract tool calls in standard format with name attribute
fn extract_standard_format(xml_text: &str) -> Option<serde_json::Value> {
    let mut reader = Reader::from_str(xml_text);
    reader.trim_text(true);

    let mut state = ParseState::Idle;
    let mut current_name = String::new();
    let mut current_args = String::new();
    let mut last_valid_call: Option<(String, String)> = None;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                match tag_name.as_str() {
                    "tool_call" => {
                        // Extract name attribute
                        if let Some(name_attr) = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .find(|attr| attr.key.as_ref() == b"name")
                        {
                            if let Ok(name) = String::from_utf8(name_attr.value.to_vec()) {
                                current_name = name;
                                state = ParseState::InsideToolCall(current_name.clone());
                            }
                        }
                    }
                    "arguments" => {
                        if matches!(state, ParseState::InsideToolCall(_)) {
                            state = ParseState::InsideArguments(current_name.clone());
                            current_args.clear();
                        }
                    }
                    _ => {} // Ignore unknown tags
                }
            }
            Ok(Event::Text(e)) => {
                if matches!(state, ParseState::InsideArguments(_)) {
                    // Accumulate text content
                    if let Ok(text) = e.unescape() {
                        current_args.push_str(&text);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                match tag_name.as_str() {
                    "arguments" => {
                        if matches!(state, ParseState::InsideArguments(_)) {
                            state = ParseState::ArgumentsCollected(
                                current_name.clone(),
                                current_args.trim().to_string(),
                            );
                        }
                    }
                    "tool_call" => {
                        // Finalize this tool call
                        if let ParseState::ArgumentsCollected(name, args) = &state {
                            last_valid_call = Some((name.clone(), args.clone()));
                        }
                        // Reset for next potential tool_call
                        state = ParseState::Idle;
                        current_name.clear();
                        current_args.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => {
                // Tolerate parse errors, just skip
                buf.clear();
                continue;
            }
            _ => {}
        }

        buf.clear();
    }

    // Convert the last valid call to OpenAI format
    last_valid_call.map(|(name, args)| {
        let normalized_args = normalize_arguments(&args);

        json!({
            "id": "call_xml_1",
            "type": "function",
            "function": {
                "name": name,
                "arguments": normalized_args
            }
        })
    })
}

/// Normalize argument text into a JSON-safe string.
///
/// Strategy:
/// 1. Try to parse as JSON (validates structure)
/// 2. If valid, serialize back to compact string
/// 3. If invalid, wrap as a JSON object with the raw text
fn normalize_arguments(args_text: &str) -> String {
    let trimmed = args_text.trim();

    // Empty args → empty object
    if trimmed.is_empty() {
        return "{}".to_string();
    }

    // Try to parse as JSON
    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(json_value) => {
            // Valid JSON, re-serialize to ensure consistent formatting
            serde_json::to_string(&json_value).unwrap_or_else(|_| "{}".to_string())
        }
        Err(_) => {
            // Invalid JSON, wrap it
            json!({ "raw": trimmed }).to_string()
        }
    }
}

/// Extract tool calls in GLM-4.6 format with arg_key/arg_value pairs
fn extract_glm46_format(xml_text: &str) -> Option<serde_json::Value> {
    let mut reader = Reader::from_str(xml_text);
    reader.trim_text(true);

    let mut in_tool_call = false;
    let mut tool_name = String::new();
    let mut current_key = String::new();
    let mut current_value = String::new();
    let mut in_arg_key = false;
    let mut in_arg_value = false;
    let mut args_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                match tag_name.as_str() {
                    "tool_call" => {
                        in_tool_call = true;
                        tool_name.clear();
                        args_map.clear();
                    }
                    "arg_key" => {
                        in_arg_key = true;
                        current_key.clear();
                    }
                    "arg_value" => {
                        in_arg_value = true;
                        current_value.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                if let Ok(text) = e.unescape() {
                    let trimmed = text.trim();
                    if in_arg_key {
                        current_key.push_str(trimmed);
                    } else if in_arg_value {
                        current_value.push_str(trimmed);
                    } else if in_tool_call && tool_name.is_empty() {
                        // First text inside tool_call is the function name
                        tool_name.push_str(trimmed);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                match tag_name.as_str() {
                    "arg_key" => {
                        in_arg_key = false;
                    }
                    "arg_value" => {
                        in_arg_value = false;
                        // Store the key-value pair
                        if !current_key.is_empty() {
                            args_map.insert(current_key.clone(), current_value.clone());
                        }
                    }
                    "tool_call" => {
                        if in_tool_call && !tool_name.is_empty() {
                            // Convert args_map to JSON
                            let args_json = serde_json::to_string(&args_map)
                                .unwrap_or_else(|_| "{}".to_string());

                            return Some(json!({
                                "id": "call_xml_glm46_1",
                                "type": "function",
                                "function": {
                                    "name": tool_name,
                                    "arguments": args_json
                                }
                            }));
                        }
                        in_tool_call = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => {
                buf.clear();
                continue;
            }
            _ => {}
        }

        buf.clear();
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_tool_call() {
        let xml = r#"<tool_call name="list_directory">
<arguments>{"path": "."}</arguments>
</tool_call>"#;

        let result = extract_tool_call_from_xml(xml);
        assert!(result.is_some());

        let call = result.unwrap();
        assert_eq!(call["type"], "function");
        assert_eq!(call["function"]["name"], "list_directory");
        assert!(call["function"]["arguments"]
            .as_str()
            .unwrap()
            .contains("path"));
    }

    #[test]
    fn test_embedded_in_reasoning() {
        let xml = r#"<thinking>
Let me analyze this...
</thinking>
<tool_call name="read_file">
<arguments>{"file": "test.txt"}</arguments>
</tool_call>
<conclusion>Done</conclusion>"#;

        let result = extract_tool_call_from_xml(xml);
        assert!(result.is_some());

        let call = result.unwrap();
        assert_eq!(call["function"]["name"], "read_file");
    }

    #[test]
    fn test_multiple_tool_calls_takes_last() {
        let xml = r#"<tool_call name="first">
<arguments>{"a": 1}</arguments>
</tool_call>
<tool_call name="second">
<arguments>{"b": 2}</arguments>
</tool_call>"#;

        let result = extract_tool_call_from_xml(xml);
        assert!(result.is_some());

        let call = result.unwrap();
        assert_eq!(call["function"]["name"], "second");
    }

    #[test]
    fn test_malformed_json_arguments() {
        let xml = r#"<tool_call name="test">
<arguments>not valid json</arguments>
</tool_call>"#;

        let result = extract_tool_call_from_xml(xml);
        assert!(result.is_some());

        let call = result.unwrap();
        let args_str = call["function"]["arguments"].as_str().unwrap();
        assert!(args_str.contains("raw"));
    }

    #[test]
    fn test_empty_arguments() {
        let xml = r#"<tool_call name="test">
<arguments></arguments>
</tool_call>"#;

        let result = extract_tool_call_from_xml(xml);
        assert!(result.is_some());

        let call = result.unwrap();
        assert_eq!(call["function"]["arguments"], "{}");
    }

    #[test]
    fn test_no_tool_call() {
        let xml = "<thinking>Just thinking, no tools</thinking>";

        let result = extract_tool_call_from_xml(xml);
        assert!(result.is_none());
    }
}
