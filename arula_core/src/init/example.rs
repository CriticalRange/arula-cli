//! Example usage of the project learning system
//!
//! This file demonstrates how to use the Project Learning System
//! to understand projects through structured discovery.

use crate::api::agent_client::AgentClient;
use crate::api::agent::AgentOptionsBuilder;
use crate::init::{InitSystem, SbpFiles};
use crate::utils::config::Config;
use anyhow::Result;

/// Example project learning
pub async fn example_learn_project() -> Result<SbpFiles> {
    // Create agent client
    let config = Config::default();
    let agent_options = AgentOptionsBuilder::new()
        .system_prompt("You are a project learning and discovery assistant.")
        .auto_execute_tools(false)
        .build();

    let agent_client = AgentClient::new(
        "openai".to_string(),
        "https://api.openai.com/v1".to_string(),
        "your-api-key".to_string(),
        "gpt-4".to_string(),
        agent_options,
        &config,
    );

    // Create learning system
    let learning_system = InitSystem::new(agent_client, config);

    // Initial understanding of the project
    let initial_understanding = "A web API for task management with user authentication, real-time updates, and data persistence";
    let project_path = "./my-project";

    // Learn about the project
    let understanding = learning_system.learn_about_project(initial_understanding, project_path).await?;

    // Generate analysis report
    let report = learning_system.generate_analysis_report(&understanding)?;

    Ok(report)
}