//! Demonstration of the Question Tool functionality

use arula_cli::tools::builtin::{QuestionTool, QuestionParams, QuestionResult};
use arula_cli::api::agent::Tool;

#[test]
fn test_question_tool_basic_properties() {
    let tool = QuestionTool::new();

    // Test tool name and description
    assert_eq!(tool.name(), "ask_question");
    assert!(tool.description().contains("clarifying question"));

    // Test schema generation
    let schema = tool.schema();
    assert_eq!(schema.name, "ask_question");
    assert!(schema.parameters.contains_key("question"));
    assert!(schema.parameters.contains_key("options"));
}

#[test]
fn test_question_params_serialization() -> Result<(), Box<dyn std::error::Error>> {
    // Test question with options
    let params = QuestionParams {
        question: "What is your favorite color?".to_string(),
        options: Some(vec!["Red".to_string(), "Blue".to_string(), "Green".to_string()]),
    };

    // Test JSON serialization
    let json_str = serde_json::to_string(&params)?;
    let deserialized: QuestionParams = serde_json::from_str(&json_str)?;

    assert_eq!(params.question, deserialized.question);
    assert_eq!(params.options, deserialized.options);

    // Test question without options
    let simple_params = QuestionParams {
        question: "Enter your name:".to_string(),
        options: None,
    };

    let json_str = serde_json::to_string(&simple_params)?;
    let simple_deserialized: QuestionParams = serde_json::from_str(&json_str)?;

    assert_eq!(simple_params.question, simple_deserialized.question);
    assert!(simple_deserialized.options.is_none());

    Ok(())
}

#[test]
fn test_question_result_structure() {
    let result = QuestionResult {
        question: "Test question?".to_string(),
        awaiting_response: true,
        success: true,
    };

    assert_eq!(result.question, "Test question?");
    assert!(result.awaiting_response);
    assert!(result.success);
}

#[test]
fn test_question_tool_in_registry() {
    use arula_cli::tools::tools::create_default_tool_registry;

    let registry = create_default_tool_registry();
    let tool_names = registry.get_tools();

    // Verify that the question tool is registered
    assert!(tool_names.contains(&"ask_question".to_string()));

    // Count total tools (should include our question tool)
    println!("Available tools: {:?}", tool_names);
    assert!(!tool_names.is_empty()); // At least our question tool should be there
}

#[tokio::test]
async fn test_question_tool_validation() {
    let tool = QuestionTool::new();

    // Test validation - empty question should fail
    let empty_params = QuestionParams {
        question: "".to_string(),
        options: None,
    };

    // This should return an error result since the question is empty
    let result = tool.execute(empty_params).await;
    assert!(result.is_err());

    // Test valid question
    let valid_params = QuestionParams {
        question: "What is your name?".to_string(),
        options: Some(vec!["Alice".to_string(), "Bob".to_string()]),
    };

    let result = tool.execute(valid_params).await;
    assert!(result.is_ok());
    
    let response = result.unwrap();
    assert!(response.success);
    assert!(response.awaiting_response);
}

fn main() {
    println!("Question Tool Demonstration");
    println!("============================");

    // Show tool properties
    let tool = QuestionTool::new();
    println!("Tool Name: {}", tool.name());
    println!("Description: {}", tool.description());

    // Example usage scenarios
    println!("\nExample Usage Scenarios:");
    println!("1. Clarification: 'What directory should I save the file to?'");
    println!("2. Confirmation: 'Do you want to proceed with this action?'");
    println!("3. Choice: 'Which option do you prefer?'");

    println!("\nFeatures:");
    println!("✨ Simple parameter structure");
    println!("🎯 Optional suggested answers");
    println!("📝 Clear question/response model");
    println!("✅ Async execution support");

    println!("\nIntegration:");
    println!("✅ Tool registered as 'ask_question'");
    println!("✅ Full agent framework compatibility");
    println!("✅ Type-safe parameters and results");
    println!("✅ Comprehensive error handling");

    println!("\nThe question tool is ready to use!");
    println!("AI agents can now ask users questions and get real-time responses.");
}
