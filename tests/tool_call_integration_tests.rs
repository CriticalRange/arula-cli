//! Integration tests for tool execution functionality

use arula_cli::tool_call::{execute_bash_tool, ToolCallResult, BashToolParams};
use serde_json;

#[tokio::test]
async fn test_bash_tool_basic_execution() -> Result<(), Box<dyn std::error::Error>> {
    // Test simple echo command
    let result = execute_bash_tool("echo 'Hello, World!'").await?;

    assert_eq!(result.tool, "bash_tool");
    assert!(result.success);
    assert!(result.output.contains("Hello, World!"));

    Ok(())
}

#[tokio::test]
async fn test_bash_tool_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    // Test command that should fail
    let result = execute_bash_tool("exit 1").await?;

    assert_eq!(result.tool, "bash_tool");
    assert!(!result.success);
    assert!(result.output.contains("Error:"));

    Ok(())
}

#[test]
fn test_bash_tool_serialization() -> Result<(), Box<dyn std::error::Error>> {
    let params = BashToolParams {
        command: "ls -la".to_string(),
    };

    // Test JSON serialization
    let json_str = serde_json::to_string(&params)?;
    let deserialized: BashToolParams = serde_json::from_str(&json_str)?;

    assert_eq!(params.command, deserialized.command);

    // Test YAML serialization
    let yaml_str = serde_yaml::to_string(&params)?;
    let yaml_deserialized: BashToolParams = serde_yaml::from_str(&yaml_str)?;

    assert_eq!(params.command, yaml_deserialized.command);

    Ok(())
}

#[test]
fn test_tool_call_result_properties() {
    let success_result = ToolCallResult {
        tool: "test_tool".to_string(),
        success: true,
        output: "Command executed successfully".to_string(),
    };

    let error_result = ToolCallResult {
        tool: "test_tool".to_string(),
        success: false,
        output: "Error: Command failed".to_string(),
    };

    // Test success result
    assert_eq!(success_result.tool, "test_tool");
    assert!(success_result.success);
    assert!(success_result.output.contains("successfully"));

    // Test error result
    assert_eq!(error_result.tool, "test_tool");
    assert!(!error_result.success);
    assert!(error_result.output.contains("Error:"));
}

#[tokio::test]
async fn test_bash_tool_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
    // Test with empty command
    let result = execute_bash_tool("").await?;
    assert_eq!(result.tool, "bash_tool");

    // Test with command that produces no output
    let result = execute_bash_tool("true").await?;
    assert_eq!(result.tool, "bash_tool");
    assert!(result.success);

    // Test with command that produces a lot of output
    let result = execute_bash_tool("echo 'Large output test'; seq 1 100").await?;
    assert_eq!(result.tool, "bash_tool");
    assert!(result.success);
    assert!(result.output.lines().count() > 100); // Should have many lines

    Ok(())
}

#[tokio::test]
async fn test_bash_tool_with_complex_commands() -> Result<(), Box<dyn std::error::Error>> {
    // Test command with pipes
    let result = execute_bash_tool("echo 'hello world' | tr '[:lower:]' '[:upper:]'").await?;
    assert!(result.success);
    assert!(result.output.contains("HELLO WORLD"));

    // Test command with variables
    let result = execute_bash_tool("VAR=test; echo $VAR").await?;
    assert!(result.success);
    assert!(result.output.contains("test"));

    Ok(())
}

#[test]
fn test_tool_parameter_validation() {
    // Test valid parameters
    let valid_params = BashToolParams {
        command: "echo 'valid command'".to_string(),
    };
    assert!(!valid_params.command.is_empty());

    // Test empty command
    let empty_params = BashToolParams {
        command: "".to_string(),
    };
    assert!(empty_params.command.is_empty());

    // Test very long command
    let long_command = "x".repeat(1000);
    let long_params = BashToolParams {
        command: long_command.clone(),
    };
    assert_eq!(long_params.command.len(), 1000);
}

#[tokio::test]
async fn test_bash_tool_concurrent_execution() -> Result<(), Box<dyn std::error::Error>> {
    use tokio::join;

    // Execute multiple commands concurrently
    let (result1, result2, result3) = join!(
        execute_bash_tool("echo 'task1'"),
        execute_bash_tool("echo 'task2'"),
        execute_bash_tool("echo 'task3'")
    );

    let result1 = result1?;
    let result2 = result2?;
    let result3 = result3?;

    // Verify all results
    for (index, result) in [result1, result2, result3].into_iter().enumerate() {
        assert_eq!(result.tool, "bash_tool");
        assert!(result.success);
        assert!(result.output.contains(&format!("task{}", index + 1)));
    }

    Ok(())
}

#[tokio::test]
async fn test_bash_tool_timeout_behavior() -> Result<(), Box<dyn std::error::Error>> {
    // Note: This test depends on the actual timeout implementation
    // For now, we'll test a quick command that should complete
    let result = execute_bash_tool("echo 'quick test'").await?;

    assert_eq!(result.tool, "bash_tool");
    assert!(result.success);
    assert!(result.output.contains("quick test"));

    Ok(())
}

#[tokio::test]
async fn test_bash_tool_special_characters() -> Result<(), Box<dyn std::error::Error>> {
    // Test command with special characters that don't break bash
    let special_chars = "!@#$%^&*()[]{}|\\;:,<>?";
    let result = execute_bash_tool(&format!("echo '{}'", special_chars)).await?;

    assert!(result.success);
    assert!(result.output.contains(special_chars));

    // Test with Unicode characters
    let unicode_text = "Hello 世界 🚀";
    let result = execute_bash_tool(&format!("echo '{}'", unicode_text)).await?;

    assert!(result.success);
    assert!(result.output.contains(unicode_text));

    // Test with quotes using double quotes for the command
    let quoted_text = "Hello 'world' \"test\"";
    let result = execute_bash_tool(&format!("echo \"{}\"", quoted_text)).await?;

    assert!(result.success);
    assert!(result.output.contains("Hello"));

    Ok(())
}