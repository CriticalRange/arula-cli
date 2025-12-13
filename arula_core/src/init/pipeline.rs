//! AI Pipeline for learning about projects
//!
//! This module implements a discovery pipeline that helps AI understand
//! projects through structured analysis and questions.

use crate::api::agent_client::AgentClient;
use crate::init::fragments::*;
use anyhow::Result;

/// Learning and discovery guidelines
const LEARNING_GUIDELINES: &str = "
GUIDELINES:
- Ask clarifying questions to understand the project better
- Focus on learning rather than making assumptions
- Identify what we know vs what we need to discover
- Be conversational and natural in your responses
- Use markdown to structure information when helpful
- Always explain your reasoning for better understanding
- Offer suggestions based on what you learn
- Don't try to initialize - focus on understanding
";

/// Project learning and discovery pipeline
pub struct ProjectLearningPipeline {
    agent_client: AgentClient,
}

impl ProjectLearningPipeline {
    pub fn new(agent_client: AgentClient) -> Self {
        Self { agent_client }
    }

    /// Step 1: Learn project context and purpose
    pub async fn learn_context(&self, description: &str) -> Result<ProjectContext> {
        let instruction = format!(
            "{}Help me understand this project better. I want to learn about it before taking any action.

Current understanding: {}

Please help me discover:
1. What is the main purpose of this project?
2. What problem domain does it operate in?
3. What are the user's goals and objectives?
4. What business value does it provide?

Feel free to ask questions to clarify anything that's unclear. Provide a brief summary of what you understand about the project context.",
            LEARNING_GUIDELINES,
            description
        );

        let response = self.query_ai(&instruction).await?;
        self.parse_context_fragment(&response)
    }

    /// Step 2: Discover architecture and technical details
    pub async fn discover_architecture(&self, context: &ProjectContext) -> Result<ArchitectureFragment> {
        let instruction = format!(
            "{}Now that I understand the project context, help me learn about the technical architecture.

Project Purpose: {}
Problem Domain: {}

Help me discover:
1. What architectural patterns might be relevant?
2. What components or modules would be needed?
3. What technologies could be suitable?
4. What external integrations might be required?

This is about exploration and learning, not making final decisions. Ask questions to understand the technical requirements better.",
            LEARNING_GUIDELINES,
            context.purpose,
            context.problem_domain
        );

        let response = self.query_ai(&instruction).await?;
        self.parse_architecture_fragment(&response)
    }

    /// Step 3: Identify requirements and constraints
    pub async fn identify_requirements(&self, context: &ProjectContext, architecture: &ArchitectureFragment) -> Result<RequirementsFragment> {
        let instruction = format!(
            "{}Based on what we've learned so far, help me identify the requirements.

Project Purpose: {}
User Goals: {:?}
Technologies Considered: {:?}

Help me discover:
1. What functional requirements exist (what the system should do)?
2. What non-functional requirements exist (how the system should be)?
3. What constraints or limitations should we consider?
4. What assumptions are we making?

This is about understanding what needs to be built, not how to build it yet.",
            LEARNING_GUIDELINES,
            context.purpose,
            context.user_goals,
            architecture.technologies
        );

        let response = self.query_ai(&instruction).await?;
        self.parse_requirements_fragment(&response)
    }

    /// Step 4: Assess current state if it's an existing project
    pub async fn assess_current_state(&self, project_path: &str) -> Result<CurrentStateFragment> {
        let instruction = format!(
            "{}Help me understand the current state of this project at path: {}

I want to learn about:
1. What existing code or files are present?
2. What dependencies are already in use?
3. What challenges or pain points exist?
4. What recent changes have been made?

This assessment will help me understand where the project stands and what needs to be done next. If the path doesn't exist or is empty, that's valuable information too - it means we're starting fresh.",
            LEARNING_GUIDELINES,
            project_path
        );

        let response = self.query_ai(&instruction).await?;
        self.parse_current_state_fragment(&response)
    }

    /// Execute AI query
    async fn query_ai(&self, instruction: &str) -> Result<String> {
        let mut blocks = self.agent_client.query(instruction, None).await?;

        // Extract text from response blocks
        let mut content = String::new();
        use futures::StreamExt;
        while let Some(block) = blocks.next().await {
            if let crate::api::agent::ContentBlock::Text { text } = block {
                content.push_str(&text);
            }
        }

        Ok(content.trim().to_string())
    }

    /// Parse project context from AI response
    fn parse_context_fragment(&self, response: &str) -> Result<ProjectContext> {
        // For learning, we'll extract key information from the conversational response
        let mut context = ProjectContext::default();

        // Simple extraction - look for key patterns in the response
        // In a real implementation, this would be more sophisticated
        let lines: Vec<&str> = response.lines().collect();

        for line in lines {
            let line_lower = line.to_lowercase();

            // Extract purpose
            if line_lower.contains("purpose") || line_lower.contains("goal") {
                if let Some(start) = line.find(':') {
                    context.purpose = line[start + 1..].trim().to_string();
                }
            }

            // Extract problem domain
            if line_lower.contains("domain") || line_lower.contains("problem") {
                if let Some(start) = line.find(':') {
                    context.problem_domain = line[start + 1..].trim().to_string();
                }
            }

            // Extract user goals (collect all)
            if line_lower.contains("user goal") || line_lower.contains("objective") {
                if let Some(start) = line.find(':') {
                    let goals = line[start + 1..].trim().to_string();
                    context.user_goals.push(goals);
                }
            }

            // Extract business value
            if line_lower.contains("business value") || line_lower.contains("value") {
                if let Some(start) = line.find(':') {
                    context.business_value = line[start + 1..].trim().to_string();
                }
            }
        }

        // If structured extraction failed, use the whole response as purpose
        if context.purpose.is_empty() {
            context.purpose = response.chars().take(200).collect::<String>();
        }

        Ok(context)
    }

    /// Parse architecture fragment from AI response
    fn parse_architecture_fragment(&self, response: &str) -> Result<ArchitectureFragment> {
        let mut architecture = ArchitectureFragment::default();

        let lines: Vec<&str> = response.lines().collect();

        for line in lines {
            let line_lower = line.to_lowercase();

            // Extract patterns
            if line_lower.contains("pattern") {
                if let Some(start) = line.find(':') {
                    let pattern = line[start + 1..].trim().to_string();
                    architecture.patterns.push(pattern);
                }
            }

            // Extract components
            if line_lower.contains("component") || line_lower.contains("module") {
                if let Some(start) = line.find(':') {
                    let component = line[start + 1..].trim().to_string();
                    architecture.components.push(component);
                }
            }

            // Extract technologies
            if line_lower.contains("technology") || line_lower.contains("tech") {
                if let Some(start) = line.find(':') {
                    let tech = line[start + 1..].trim().to_string();
                    architecture.technologies.push(tech);
                }
            }

            // Extract integrations
            if line_lower.contains("integration") {
                if let Some(start) = line.find(':') {
                    let integration = line[start + 1..].trim().to_string();
                    architecture.integrations.push(integration);
                }
            }
        }

        Ok(architecture)
    }

    /// Parse requirements fragment from AI response
    fn parse_requirements_fragment(&self, response: &str) -> Result<RequirementsFragment> {
        let mut requirements = RequirementsFragment::default();

        let lines: Vec<&str> = response.lines().collect();

        for line in lines {
            let line_lower = line.to_lowercase();

            // Extract functional requirements
            if line_lower.contains("functional") || line_lower.contains("should do") {
                if let Some(start) = line.find(':') {
                    let req = line[start + 1..].trim().to_string();
                    requirements.functional.push(req);
                }
            }

            // Extract non-functional requirements
            if line_lower.contains("non-functional") || line_lower.contains("how the") {
                if let Some(start) = line.find(':') {
                    let req = line[start + 1..].trim().to_string();
                    requirements.non_functional.push(req);
                }
            }

            // Extract constraints
            if line_lower.contains("constraint") || line_lower.contains("limitation") {
                if let Some((key, value)) = line.split_once(':') {
                    requirements.constraints.insert(
                        key.trim().to_string(),
                        value.trim().to_string()
                    );
                }
            }

            // Extract assumptions
            if line_lower.contains("assumption") {
                if let Some(start) = line.find(':') {
                    let assumption = line[start + 1..].trim().to_string();
                    requirements.assumptions.push(assumption);
                }
            }
        }

        Ok(requirements)
    }

    /// Parse current state fragment from AI response
    fn parse_current_state_fragment(&self, response: &str) -> Result<CurrentStateFragment> {
        let mut state = CurrentStateFragment::default();

        let lines: Vec<&str> = response.lines().collect();

        for line in lines {
            let line_lower = line.to_lowercase();

            // Extract existing code
            if line_lower.contains("code") || line_lower.contains("file") {
                if let Some(start) = line.find(':') {
                    let code = line[start + 1..].trim().to_string();
                    state.existing_code.push(code);
                }
            }

            // Extract dependencies
            if line_lower.contains("dependenc") {
                if let Some(start) = line.find(':') {
                    let dep = line[start + 1..].trim().to_string();
                    state.dependencies.push(dep);
                }
            }

            // Extract pain points
            if line_lower.contains("pain point") || line_lower.contains("challenge") {
                if let Some(start) = line.find(':') {
                    let pain = line[start + 1..].trim().to_string();
                    state.pain_points.push(pain);
                }
            }

            // Extract recent changes
            if line_lower.contains("recent") || line_lower.contains("change") {
                if let Some(start) = line.find(':') {
                    let change = line[start + 1..].trim().to_string();
                    state.recent_changes.push(change);
                }
            }
        }

        Ok(state)
    }
}