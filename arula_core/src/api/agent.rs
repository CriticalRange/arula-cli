//! Modern AI Agent implementation with type-safe tool calling
//!
//! This module implements patterns inspired by open-agent-sdk but using
//! our existing reqwest-based infrastructure to avoid OpenSSL dependencies.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub data: Value,
    pub error: Option<String>,
}

impl ToolResult {
    pub fn success(data: Value) -> Self {
        Self {
            success: true,
            data,
            error: None,
        }
    }

    pub fn error(error: String) -> Self {
        Self {
            success: false,
            data: json!(null),
            error: Some(error),
        }
    }
}

/// Tool parameter schema builder
#[derive(Debug, PartialEq)]
pub struct ToolSchemaBuilder {
    name: String,
    description: String,
    parameters: HashMap<String, ParameterSchema>,
    required: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterSchema {
    pub param_type: String,
    pub description: String,
    pub required: bool,
    pub default: Option<Value>,
    pub enum_values: Option<Vec<Value>>,
}

impl ToolSchemaBuilder {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            parameters: HashMap::new(),
            required: Vec::new(),
        }
    }

    pub fn param(mut self, name: &str, param_type: &str) -> Self {
        self.parameters.insert(
            name.to_string(),
            ParameterSchema {
                param_type: param_type.to_string(),
                description: String::new(),
                required: false,
                default: None,
                enum_values: None,
            },
        );
        self
    }

    pub fn description(mut self, name: &str, description: &str) -> Self {
        if let Some(param) = self.parameters.get_mut(name) {
            param.description = description.to_string();
        }
        self
    }

    pub fn required(mut self, name: &str) -> Self {
        if let Some(param) = self.parameters.get_mut(name) {
            param.required = true;
        }
        if !self.required.contains(&name.to_string()) {
            self.required.push(name.to_string());
        }
        self
    }

    pub fn default(mut self, name: &str, default: Value) -> Self {
        if let Some(param) = self.parameters.get_mut(name) {
            param.default = Some(default);
        }
        self
    }

    pub fn enum_values(mut self, name: &str, values: Vec<Value>) -> Self {
        if let Some(param) = self.parameters.get_mut(name) {
            param.enum_values = Some(values);
        }
        self
    }

    pub fn build(self) -> ToolSchema {
        ToolSchema {
            name: self.name,
            description: self.description,
            parameters: self.parameters,
            required: self.required,
        }
    }
}

/// Tool schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: HashMap<String, ParameterSchema>,
    pub required: Vec<String>,
}

impl ToolSchema {
    pub fn to_openai_schema(&self) -> Value {
        let mut properties = serde_json::Map::new();

        for (name, param) in &self.parameters {
            let mut param_obj = serde_json::Map::new();
            param_obj.insert("type".to_string(), json!(param.param_type));
            param_obj.insert("description".to_string(), json!(param.description));

            if let Some(default) = &param.default {
                param_obj.insert("default".to_string(), default.clone());
            }

            if let Some(enum_values) = &param.enum_values {
                param_obj.insert("enum".to_string(), json!(enum_values));
            }

            properties.insert(name.clone(), json!(param_obj));
        }

        json!({
            "type": "object",
            "properties": properties,
            "required": self.required
        })
    }

    pub fn to_openai_tool(&self) -> Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.to_openai_schema()
            }
        })
    }
}

/// Async trait for tools
#[async_trait]
pub trait Tool: Send + Sync {
    type Params: for<'de> Deserialize<'de> + Send;
    type Result: Serialize + Send;

    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> ToolSchema;

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String>;

    async fn execute_with_result(&self, params: Value) -> ToolResult {
        match serde_json::from_value::<Self::Params>(params) {
            Ok(typed_params) => match self.execute(typed_params).await {
                Ok(result) => {
                    let json_result = serde_json::to_value(&result)
                        .unwrap_or_else(|_e| json!("Failed to serialize result"));
                    ToolResult::success(json_result)
                }
                Err(error) => ToolResult::error(error),
            },
            Err(error) => ToolResult::error(format!("Invalid parameters: {}", error)),
        }
    }
}

/// Tool registry for managing available tools
#[derive(Clone)]
pub struct ToolRegistry {
    tools: std::sync::Arc<
        std::sync::RwLock<
            HashMap<
                String,
                std::sync::Arc<dyn Tool<Params = serde_json::Value, Result = serde_json::Value>>,
            >,
        >,
    >,
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.tools.read().unwrap().len();
        f.debug_struct("ToolRegistry")
            .field("tool_count", &count)
            .finish()
    }
}

// Removing PartialEq as it's difficult to implement correctly with RwLock and likely unnecessary

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: std::sync::Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        let name = tool.name().to_string();
        // Convert to trait object with generic type erasure
        let arc_tool: std::sync::Arc<
            dyn Tool<Params = serde_json::Value, Result = serde_json::Value>,
        > = std::sync::Arc::new(GenericToolWrapper::new(tool));
        self.tools.write().unwrap().insert(name, arc_tool);
    }

    pub fn get_tools(&self) -> Vec<String> {
        self.tools.read().unwrap().keys().cloned().collect()
    }

    pub fn get_openai_tools(&self) -> Vec<Value> {
        self.tools
            .read()
            .unwrap()
            .values()
            .map(|tool| tool.schema().to_openai_tool())
            .collect()
    }

    pub async fn execute_tool(&self, name: &str, params: Value) -> Option<ToolResult> {
        let tool = { self.tools.read().unwrap().get(name).cloned() };

        if let Some(tool) = tool {
            Some(tool.execute_with_result(params).await)
        } else {
            None
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrapper to convert specific Tool implementations to generic ones
struct GenericToolWrapper<T> {
    inner: T,
}

impl<T> GenericToolWrapper<T> {
    fn new(tool: T) -> Self {
        Self { inner: tool }
    }
}

#[async_trait]
impl<T> Tool for GenericToolWrapper<T>
where
    T: Tool + Send + Sync + 'static,
{
    type Params = serde_json::Value;
    type Result = serde_json::Value;

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn schema(&self) -> ToolSchema {
        self.inner.schema()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        // Convert the generic Value params to the specific tool's Params type
        let typed_params = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return Err(format!("Parameter conversion failed: {}", e)),
        };

        // Call the inner tool's execute method
        let result = self.inner.execute(typed_params).await;

        // Convert the specific result to Value - unwrap the Result first!
        match result {
            Ok(value) => {
                serde_json::to_value(value).map_err(|e| format!("Result conversion failed: {}", e))
            }
            Err(e) => Err(e),
        }
    }
}

/// Agent configuration builder
pub struct AgentOptionsBuilder {
    system_prompt: Option<String>,
    model: Option<String>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    auto_execute_tools: bool,
    max_tool_iterations: u32,
    debug: bool,
    streaming: bool,
}

impl Default for AgentOptionsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentOptionsBuilder {
    pub fn new() -> Self {
        Self {
            system_prompt: None,
            model: None,
            temperature: None,
            max_tokens: None,
            auto_execute_tools: true,
            max_tool_iterations: 50,
            debug: false,
            streaming: true,
        }
    }

    pub fn system_prompt(mut self, prompt: &str) -> Self {
        self.system_prompt = Some(prompt.to_string());
        self
    }

    pub fn model(mut self, model: &str) -> Self {
        self.model = Some(model.to_string());
        self
    }

    pub fn temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn auto_execute_tools(mut self, auto_execute: bool) -> Self {
        self.auto_execute_tools = auto_execute;
        self
    }

    pub fn max_tool_iterations(mut self, max_iterations: u32) -> Self {
        self.max_tool_iterations = max_iterations;
        self
    }

    pub fn debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    pub fn build(self) -> AgentOptions {
        AgentOptions {
            system_prompt: self
                .system_prompt
                .unwrap_or_else(|| "You are a helpful AI assistant.".to_string()),
            model: self.model.unwrap_or_else(|| "gpt-3.5-turbo".to_string()),
            temperature: self.temperature.unwrap_or(0.7),
            max_tokens: self.max_tokens.unwrap_or(2048),
            auto_execute_tools: self.auto_execute_tools,
            max_tool_iterations: self.max_tool_iterations,
            debug: self.debug,
            streaming: self.streaming,
        }
    }
}

/// Agent configuration
#[derive(Debug, Clone)]
pub struct AgentOptions {
    pub system_prompt: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub auto_execute_tools: bool,
    pub max_tool_iterations: u32,
    pub debug: bool,
    pub streaming: bool,
}

impl Default for AgentOptions {
    fn default() -> Self {
        AgentOptionsBuilder::new().build()
    }
}

/// Content block for streaming responses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Reasoning {
        reasoning: String,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    ToolResult {
        tool_call_id: String,
        result: ToolResult,
    },
    BashOutputLine {
        tool_call_id: String,
        line: String,
        is_stderr: bool,
    },
    AskQuestion {
        tool_call_id: String,
        question: String,
        options: Option<Vec<String>>,
    },
    Error {
        error: String,
    },
}

impl ContentBlock {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn reasoning(reasoning: impl Into<String>) -> Self {
        Self::Reasoning {
            reasoning: reasoning.into(),
        }
    }

    pub fn tool_call(id: String, name: String, arguments: String) -> Self {
        Self::ToolCall {
            id,
            name,
            arguments,
        }
    }

    pub fn tool_result(tool_call_id: String, result: ToolResult) -> Self {
        Self::ToolResult {
            tool_call_id,
            result,
        }
    }

    pub fn error(error: impl Into<String>) -> Self {
        Self::Error {
            error: error.into(),
        }
    }
}
