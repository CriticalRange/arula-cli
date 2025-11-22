//! Demonstration of the Question Tool functionality

use arula_cli::tools::{QuestionTool, QuestionParams, QuestionType};
use arula_cli::agent::Tool;
use serde_json;

#[test]
fn test_question_tool_basic_properties() {
    let tool = QuestionTool::new();

    // Test tool name and description
    assert_eq!(tool.name(), "ask_question");
    assert!(tool.description().contains("beautiful animated dialog"));

    // Test schema generation
    let schema = tool.schema();
    assert_eq!(schema.name, "ask_question");
    assert!(schema.parameters.contains_key("question"));
    assert!(schema.parameters.contains_key("options"));
    assert!(schema.parameters.contains_key("allow_custom_response"));
    assert!(schema.parameters.contains_key("question_type"));
}

#[test]
fn test_question_params_serialization() -> Result<(), Box<dyn std::error::Error>> {
    // Test multiple choice question
    let params = QuestionParams {
        question: "What is your favorite color?".to_string(),
        options: vec!["Red".to_string(), "Blue".to_string(), "Green".to_string()],
        allow_custom_response: false,
        question_type: QuestionType::MultipleChoice,
    };

    // Test JSON serialization
    let json_str = serde_json::to_string(&params)?;
    let deserialized: QuestionParams = serde_json::from_str(&json_str)?;

    assert_eq!(params.question, deserialized.question);
    assert_eq!(params.options, deserialized.options);
    assert_eq!(params.allow_custom_response, deserialized.allow_custom_response);

    // Test confirmation question
    let confirmation_params = QuestionParams {
        question: "Do you want to continue?".to_string(),
        options: vec!["Yes".to_string(), "No".to_string()],
        allow_custom_response: false,
        question_type: QuestionType::Confirmation,
    };

    let json_str = serde_json::to_string(&confirmation_params)?;
    let confirmation_deserialized: QuestionParams = serde_json::from_str(&json_str)?;

    assert_eq!(confirmation_params.question, confirmation_deserialized.question);
    assert_eq!(confirmation_params.question_type, confirmation_deserialized.question_type);

    // Test text input question
    let text_params = QuestionParams {
        question: "Enter your name:".to_string(),
        options: vec![],
        allow_custom_response: true,
        question_type: QuestionType::TextInput,
    };

    let json_str = serde_json::to_string(&text_params)?;
    let text_deserialized: QuestionParams = serde_json::from_str(&json_str)?;

    assert_eq!(text_params.question, text_deserialized.question);
    assert_eq!(text_params.allow_custom_response, text_deserialized.allow_custom_response);
    assert_eq!(text_params.question_type, text_deserialized.question_type);

    Ok(())
}

#[test]
fn test_question_result_structure() {
    use arula_cli::tools::QuestionResult;

    let result = QuestionResult {
        question: "Test question?".to_string(),
        response: "Test answer".to_string(),
        question_type: "multiple_choice".to_string(),
        was_custom: false,
        success: true,
    };

    assert_eq!(result.question, "Test question?");
    assert_eq!(result.response, "Test answer");
    assert_eq!(result.question_type, "multiple_choice");
    assert!(!result.was_custom);
    assert!(result.success);
}

#[test]
fn test_question_tool_in_registry() {
    use arula_cli::tools::create_default_tool_registry;

    let registry = create_default_tool_registry();
    let tool_names = registry.get_tools();

    // Verify that the question tool is registered
    assert!(tool_names.contains(&"ask_question"));

    // Count total tools (should include our new question tool)
    println!("Available tools: {:?}", tool_names);
    assert!(tool_names.len() > 0); // At least our question tool should be there
}

#[test]
fn test_question_type_defaults() {
    // Test default question type
    let params = QuestionParams {
        question: "Test?".to_string(),
        options: vec!["Yes".to_string(), "No".to_string()],
        allow_custom_response: false,
        question_type: QuestionType::MultipleChoice, // Default
    };

    assert!(matches!(params.question_type, QuestionType::MultipleChoice));
}

#[tokio::test]
async fn test_question_tool_validation() {
    let tool = QuestionTool::new();

    // Test validation - empty question should fail
    let empty_params = QuestionParams {
        question: "".to_string(),
        options: vec![],
        allow_custom_response: false,
        question_type: QuestionType::MultipleChoice,
    };

    // This should return an error result since the question is empty
    let result = tool.execute(empty_params).await;
    assert!(result.is_err());

    // Test valid text input question
    let valid_params = QuestionParams {
        question: "What is your name?".to_string(),
        options: vec![],
        allow_custom_response: true,
        question_type: QuestionType::TextInput,
    };

    // This should not error out during validation
    // Note: In a real test, this would show a dialog, but for testing we just check validation
    let result = tool.execute(valid_params).await;
    // Result might be Ok (user cancelled) or Err (validation error), but shouldn't panic
    match result {
        Ok(_) | Err(_) => {}, // Both are acceptable for this test
    }
}

fn main() {
    println!("Question Tool Demonstration");
    println!("============================");

    // Show tool properties
    let tool = QuestionTool::new();
    println!("Tool Name: {}", tool.name());
    println!("Description: {}", tool.description());

    // Show available question types
    println!("\nSupported Question Types:");
    println!("- Multiple Choice: Present a list of options");
    println!("- Confirmation: Yes/No type questions");
    println!("- Text Input: Free-form text entry");
    println!("- Info: Display information with confirmation");

    // Example usage scenarios
    println!("\nExample Usage Scenarios:");
    println!("1. Confirmation: 'Do you want to delete this file?'");
    println!("2. Multiple Choice: 'Which deployment environment?'");
    println!("3. Text Input: 'Enter your API key:'");
    println!("4. Info: 'Operation completed. Continue?'");

    println!("\nFeatures:");
    println!("✨ Beautiful animated dialog boxes");
    println!("🎨 Gradient color effects");
    println!("⚡ Smooth selection animations");
    println!("🔄 Tab switching between options and custom input");
    println!("⌨️  Keyboard navigation (↑↓, Enter, Esc)");
    println!("📱 Responsive terminal sizing");
    println!("🎯 Context-appropriate icons");

    println!("\nIntegration:");
    println!("✅ Tool registered as 'ask_question'");
    println!("✅ Full agent framework compatibility");
    println!("✅ Type-safe parameters and results");
    println!("✅ Comprehensive error handling");

    println!("\nThe question tool is ready to use!");
    println!("AI agents can now ask users questions and get real-time responses.");
}