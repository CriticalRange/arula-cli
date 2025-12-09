//! Arula Desktop - Entry point for the Iced GUI application.

use arula_core::utils::config::Config;
use arula_core::SessionConfig;
use arula_desktop::canvas::{LiquidMenuBackground, LivingBackground};
use arula_desktop::styles::{
    ai_bubble_style, chat_input_container_style, chat_input_style,
    cog_button_container_style_button, input_style, primary_button_style, send_button_style,
    transparent_style, user_bubble_style,
};
use arula_desktop::animation::Spring;
use arula_desktop::{
    app_theme, collect_provider_options, palette, ConfigForm, Dispatcher, LiquidMenuState,
    LivingBackgroundState, MessageEntry, PaletteColors, Session, SettingsMenuState, SettingsPage,
    TiltCardState, UiEvent, MENU_BUTTON_SIZE, MESSAGE_MAX_WIDTH, PAGE_SLIDE_DISTANCE,
    SETTINGS_CARD_WIDTH, TICK_INTERVAL_MS, TILT_CARD_COUNT,
};
use iced_fonts::Bootstrap;

use chrono::Utc;
use iced::alignment::{Horizontal, Vertical};
use iced::time::{self, Duration};
use iced::widget::canvas::Canvas;
use iced::widget::text_editor;
use iced::widget::text_input::Id as TextInputId;
use iced::widget::{
    button, column, container, markdown, pick_list, row, scrollable, stack, text, text_input, Space,
};
use iced::{Background, Border, Color, Element, Font, Length, Point, Subscription, Task};
use std::collections::HashMap;

/// Application state.
struct App {
    dispatcher: Dispatcher,
    sessions: Vec<Session>,
    current: usize,
    draft: String,
    config: Config,
    config_form: ConfigForm,
    bg_state: LivingBackgroundState,
    /// Opacity for the living background (0.0 = disabled/gray, 1.0 = enabled)
    bg_opacity: f32,
    menu_state: LiquidMenuState,
    /// Settings submenu navigation state
    settings_state: SettingsMenuState,
    /// Tilt card states (uses Vec to eliminate duplicate fields)
    tilt_cards: Vec<TiltCardState>,
    /// Error message if initialization failed
    init_error: Option<String>,
    /// Editor contents for each message (keyed by session_index:message_index)
    message_editors: HashMap<String, text_editor::Content>,
    /// Cached model list for model selector
    model_list: Vec<String>,
    /// Whether models are currently being fetched
    models_loading: bool,
    /// Cached parsed markdown for AI messages (keyed by session_index:message_index)
    markdown_cache: HashMap<String, Vec<markdown::Item>>,
    /// Track tool display args from ToolCallStart to show in ToolCallResult (keyed by session_id)
    tool_args_cache: HashMap<uuid::Uuid, String>,
    /// Track expand/collapse animation state for tool messages (keyed by "session_index:message_index")
    /// Spring position: 0.0 = collapsed, 1.0 = expanded
    tool_animations: HashMap<String, Spring>,
    /// Current stream error to display to the user
    stream_error: Option<String>,
    /// Whether the error toast is expanded to show full error
    error_expanded: bool,
    /// Streaming bash output lines per tool call (keyed by tool_call_id)
    bash_output_lines: HashMap<String, Vec<(String, bool)>>, // (line, is_stderr)
}

/// Application messages.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Some variants reserved for future features
enum Message {
    DraftChanged(String),
    SendPrompt,
    Received(UiEvent),
    NewTab,
    ToggleSettings,
    CloseSettings,
    Tick,
    ConfigProviderChanged(String),
    ConfigModelChanged(String),
    ConfigStreamingToggled(bool),
    ConfigLivingBackgroundToggled(bool),
    ConfigApiUrlChanged(String),
    /// Handle z.ai endpoint selection change
    ConfigEndpointChanged(String),
    ConfigApiKeyChanged(String),
    ConfigThinkingToggled(bool),
    ConfigWebSearchToggled(bool),
    ConfigOllamaToolsToggled(bool),
    ConfigSystemPromptChanged(String),
    ConfigTemperatureChanged(f32),
    ConfigMaxTokensChanged(String),
    SaveConfig,
    CardHovered(usize, bool),
    CardMouseMoved(usize, Point),
    /// Handle text editor actions for message selection
    MessageEditorAction(String, text_editor::Action),
    /// Navigate to a settings submenu page
    SettingsNavigate(SettingsPage),
    /// Navigate back to main settings page
    SettingsBack,
    /// Open model selector and start fetching models
    OpenModelSelector,
    /// Select a model from the list
    SelectModel(String),
    /// Handle markdown link clicks
    LinkClicked(markdown::Url),
    /// Stop the current streaming session
    StopStream,
    /// Toggle collapse state for a tool message bubble
    ToggleToolCollapse(String),
    /// Dismiss the error notification
    DismissError,
    /// Toggle error toast expand/collapse
    ToggleErrorExpand,
    /// Copy message content to clipboard
    CopyToClipboard(String),
}

/// Input field ID for focus management
fn input_id() -> TextInputId {
    TextInputId::new("chat-input")
}

/// Read ARULA.md from ~/.arula/ directory
fn read_global_arula_md() -> Option<String> {
    let home_dir = dirs::home_dir()?;
    let global_arula_path = home_dir.join(".arula").join("ARULA.md");

    if global_arula_path.exists() {
        std::fs::read_to_string(&global_arula_path).ok()
    } else {
        None
    }
}

/// Read ARULA.md from current directory
fn read_local_arula_md() -> Option<String> {
    let local_arula_path = std::path::Path::new("ARULA.md");

    if local_arula_path.exists() {
        std::fs::read_to_string(local_arula_path).ok()
    } else {
        None
    }
}

/// Build enhanced system prompt with ARULA.md content
fn build_enhanced_system_prompt(base_prompt: &str) -> String {
    let mut prompt_parts = Vec::new();

    // Start with base prompt
    prompt_parts.push(base_prompt.to_string());

    // Add global ARULA.md from ~/.arula/
    if let Some(global_arula) = read_global_arula_md() {
        prompt_parts.push(format!(
            "\n## Global Project Instructions\n{}",
            global_arula
        ));
    }

    // Add local ARULA.md from current directory
    if let Some(local_arula) = read_local_arula_md() {
        prompt_parts.push(format!("\n## Current Project Context\n{}", local_arula));
    }

    prompt_parts.join("\n")
}

impl App {
    /// Initializes the application. Shows error dialog if initialization fails.
    fn init() -> (Self, Task<Message>) {
        match Self::try_init() {
            // Focus the input field on startup
            Ok(app) => (app, text_input::focus(input_id())),
            Err(err) => {
                eprintln!("Initialization error: {err}");
                (Self::error_state(err.to_string()), Task::none())
            }
        }
    }

    /// Attempts to initialize the application, returning errors properly.
    fn try_init() -> anyhow::Result<Self> {
        // Initialize the global logger for debug file output
        let _ = arula_core::utils::logger::init_global_logger();

        let config = Config::load_or_default()?;
        let dispatcher = Dispatcher::new(&config)?;
        let config_form = ConfigForm::from_config(&config);
        let session = Session::new();

        // Create tilt cards using Vec instead of duplicate fields
        let tilt_cards: Vec<TiltCardState> = (0..TILT_CARD_COUNT)
            .map(|_| TiltCardState::default())
            .collect();

        let bg_opacity = if config.get_living_background_enabled() {
            1.0
        } else {
            0.0
        };

        Ok(Self {
            dispatcher,
            sessions: vec![session],
            current: 0,
            draft: String::new(),
            config,
            config_form,
            bg_state: LivingBackgroundState::default(),
            bg_opacity,
            menu_state: LiquidMenuState::default(),
            settings_state: SettingsMenuState::default(),
            tilt_cards,
            init_error: None,
            message_editors: HashMap::new(),
            model_list: Vec::new(),
            models_loading: false,
            markdown_cache: HashMap::new(),
            tool_args_cache: HashMap::new(),
            tool_animations: HashMap::new(),
            stream_error: None,
            error_expanded: false,
            bash_output_lines: HashMap::new(),
        })
    }

    fn error_state(error: String) -> Self {
        Self {
            dispatcher: Dispatcher::new(&Config::default()).unwrap(),
            sessions: vec![Session::new()],
            current: 0,
            draft: String::new(),
            config: Config::default(),
            config_form: ConfigForm::from_config(&Config::default()),
            bg_state: LivingBackgroundState::default(),
            bg_opacity: 1.0,
            menu_state: LiquidMenuState::default(),
            settings_state: SettingsMenuState::default(),
            tilt_cards: (0..TILT_CARD_COUNT)
                .map(|_| TiltCardState::default())
                .collect(),
            init_error: Some(error),
            message_editors: HashMap::new(),
            model_list: Vec::new(),
            models_loading: false,
            markdown_cache: HashMap::new(),
            tool_args_cache: HashMap::new(),
            tool_animations: HashMap::new(),
            stream_error: None,
            error_expanded: false,
            bash_output_lines: HashMap::new(),
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DraftChanged(s) => self.draft = s,
            Message::SendPrompt => {
                if let Some(session) = self.sessions.get_mut(self.current) {
                    if session.is_streaming {
                        return Task::none();
                    }
                    let prompt = std::mem::take(&mut self.draft);
                    if prompt.trim().is_empty() {
                        return Task::none();
                    }

                    session.add_user_message(prompt.clone(), Utc::now().to_rfc3339());

                    // Sync editor content for the new message
                    let msg_idx = session.messages.len() - 1;
                    let key = format!("{}:{}", self.current, msg_idx);
                    self.message_editors.insert(
                        key,
                        text_editor::Content::with_text(&session.messages[msg_idx].content),
                    );

                    session.set_streaming(true);

                    let session_config = SessionConfig {
                        system_prompt: build_enhanced_system_prompt(
                            &self.config_form.system_prompt,
                        ),
                        model: self.config.get_model(),
                        max_tokens: self.config_form.max_tokens as u32,
                        temperature: self.config_form.temperature,
                    };

                    // Get conversation history for context (excluding the current prompt which is included separately)
                    let history = session.get_chat_history();
                    let history_opt = if history.is_empty() {
                        None
                    } else {
                        Some(history)
                    };

                    if let Err(err) = self.dispatcher.start_stream(
                        session.id,
                        prompt,
                        history_opt,
                        session_config,
                    ) {
                        eprintln!("dispatch error: {err}");
                        session.set_streaming(false);
                    }
                }
                // Re-focus input after sending
                return text_input::focus(input_id());
            }
            Message::Received(ev) => return self.handle_ui_event(ev),
            Message::NewTab => {
                self.sessions.push(Session::new());
                self.current = self.sessions.len() - 1;
            }
            Message::ToggleSettings => {
                self.menu_state.open();
                self.config_form.clear_status();
            }
            Message::CloseSettings => {
                self.menu_state.close();
                // Reset settings submenu state when closing
                self.settings_state.reset();
            }
            Message::Tick => {
                self.menu_state.update();
                self.settings_state.update(); // Update settings page transitions
                self.bg_state.update();

                // Animate background opacity based on config
                // We use the *config* value (saved), not the form value, to drive the actual display
                let target = if self.config.get_living_background_enabled() {
                    1.0
                } else {
                    0.0
                };
                // Smooth fade (lerp)
                self.bg_opacity = self.bg_opacity + (target - self.bg_opacity) * 0.1;

                // Snap to target if very close
                if (self.bg_opacity - target).abs() < 0.005 {
                    self.bg_opacity = target;
                }

                // Poll for cached models if loading
                if self.models_loading {
                    let provider = self.config_form.provider.to_lowercase();
                    let cached = match provider.as_str() {
                        "openai" => self.dispatcher.get_cached_openai_models(),
                        "anthropic" => self.dispatcher.get_cached_anthropic_models(),
                        "ollama" => self.dispatcher.get_cached_ollama_models(),
                        "z.ai coding plan" | "z.ai" | "zai" => {
                            self.dispatcher.get_cached_zai_models()
                        }
                        "openrouter" => self.dispatcher.get_cached_openrouter_models(),
                        _ => None,
                    };
                    if let Some(models) = cached {
                        if !models.is_empty()
                            && !models[0].contains("Loading")
                            && !models[0].contains("Fetching")
                        {
                            self.model_list = models;
                            self.models_loading = false;
                        }
                    }
                }

                // Note: This Tick also drives the message bubble fade-in animations
                // Iced automatically redraws the view after handling a message

                // Update all tilt cards efficiently
                let mut redraw_cards = false;
                for card in &mut self.tilt_cards {
                    if card.update() {
                        redraw_cards = true;
                    }
                }
                if redraw_cards {
                    for card in &mut self.tilt_cards {
                        card.clear_cache();
                    }
                }
                
                // Update tool expand/collapse animations
                for spring in self.tool_animations.values_mut() {
                    spring.update();
                }
            }
            Message::ConfigProviderChanged(provider) => {
                // Use switch_provider to automatically set defaults (API URL, model)
                if let Err(e) = self.config.switch_provider(&provider) {
                    eprintln!("Provider switch error: {e}");
                }
                // Refresh form from updated config to show new defaults
                let options = collect_provider_options(&self.config);
                self.config_form = ConfigForm::from_config(&self.config);
                self.config_form.provider = provider;
                self.config_form.provider_options = options;
            }
            Message::ConfigModelChanged(model) => {
                self.config_form.model = model;
            }
            Message::ConfigApiUrlChanged(url) => {
                self.config_form.api_url = url;
                self.config_form.clear_status();
            }
            Message::ConfigEndpointChanged(endpoint_name) => {
                use arula_core::utils::config::ZaiEndpoint;
                self.config_form.endpoint_name = endpoint_name.clone();
                // Update api_url based on selected endpoint
                if let Some(endpoint) = ZaiEndpoint::by_name(&endpoint_name) {
                    self.config_form.api_url = endpoint.url;
                }
                self.config_form.clear_status();
            }
            Message::ConfigApiKeyChanged(key) => {
                self.config_form.api_key = key;
            }
            Message::ConfigThinkingToggled(on) => {
                self.config_form.thinking_enabled = on;
            }
            Message::ConfigWebSearchToggled(on) => {
                self.config_form.web_search_enabled = on;
            }
            Message::ConfigOllamaToolsToggled(on) => {
                self.config_form.ollama_tools_enabled = on;
            }
            Message::ConfigStreamingToggled(on) => {
                self.config_form.streaming_enabled = on;
            }
            Message::ConfigLivingBackgroundToggled(on) => {
                self.config_form.living_background_enabled = on;
            }
            Message::ConfigSystemPromptChanged(val) => {
                self.config_form.system_prompt = val;
            }
            Message::ConfigTemperatureChanged(val) => {
                self.config_form.temperature = val;
            }
            Message::ConfigMaxTokensChanged(val) => {
                if let Ok(n) = val.parse() {
                    self.config_form.max_tokens = n;
                }
            }
            Message::SaveConfig => {
                self.apply_config_changes();
            }
            // Single match arm handles all tilt cards via index
            Message::CardHovered(idx, hovered) => {
                if let Some(card) = self.tilt_cards.get_mut(idx) {
                    card.set_hovered(hovered);
                }
            }
            Message::CardMouseMoved(idx, point) => {
                if let Some(card) = self.tilt_cards.get_mut(idx) {
                    card.set_mouse_position(point);
                }
            }
            Message::MessageEditorAction(key, action) => {
                // Handle text selection actions (but filter out editing actions)
                if let Some(content) = self.message_editors.get_mut(&key) {
                    // Only allow selection-related actions, not editing
                    if action.is_edit() {
                        // Ignore edit actions to keep content read-only
                    } else {
                        content.perform(action);
                    }
                }
            }
            Message::SettingsNavigate(page) => {
                self.settings_state.navigate_to(page);
            }
            Message::SettingsBack => {
                self.settings_state.navigate_back();
            }
            Message::OpenModelSelector => {
                // Navigate to model selector page and start fetching models
                self.settings_state.navigate_to(SettingsPage::ModelSelector);
                self.models_loading = true;
                self.model_list.clear();
                // Fetch models based on current provider
                let provider = self.config_form.provider.to_lowercase();
                match provider.as_str() {
                    "openai" => self.dispatcher.fetch_openai_models(),
                    "anthropic" => self.dispatcher.fetch_anthropic_models(),
                    "ollama" => self.dispatcher.fetch_ollama_models(),
                    "z.ai coding plan" | "z.ai" | "zai" => self.dispatcher.fetch_zai_models(),
                    "openrouter" => self.dispatcher.fetch_openrouter_models(),
                    _ => {
                        self.models_loading = false;
                    }
                }
            }
            Message::SelectModel(model) => {
                self.config_form.model = model;
                // Go back to Provider page, not Main
                self.settings_state.navigate_to(SettingsPage::Provider);
            }
            Message::LinkClicked(url) => {
                // Open the URL in the default browser
                if let Err(e) = open::that(url.as_str()) {
                    eprintln!("Failed to open URL: {}", e);
                }
            }
            Message::StopStream => {
                // Stop the current streaming session
                if let Some(session) = self.sessions.get_mut(self.current) {
                    if session.is_streaming {
                        self.dispatcher.stop_stream(session.id);
                        session.set_streaming(false);
                        // Re-focus the input after stopping
                        return text_input::focus(input_id());
                    }
                }
            }
            Message::ToggleToolCollapse(key) => {
                // Get or create animation spring for this tool
                // Important: we need to know the DEFAULT state to create the spring correctly
                // Thinking bubbles (finalized) default to collapsed, tools default to expanded
                let is_thinking = key.contains(":") && self.sessions.iter().enumerate().any(|(sidx, session)| {
                    session.messages.iter().enumerate().any(|(midx, msg)| {
                        let msg_key = format!("{}:{}", sidx, midx);
                        msg_key == key && msg.is_thinking() && msg.thinking_duration_secs.is_some()
                    })
                });
                
                let spring = self.tool_animations.entry(key).or_insert_with(|| {
                    let mut s = Spring::default();
                    if is_thinking {
                        // Finalized thinking defaults to collapsed
                        s.position = 0.0;
                        s.target = 0.0;
                    } else {
                        // Tools default to expanded
                        s.position = 1.0;
                        s.target = 1.0;
                    }
                    s
                });
                
                // Toggle the target: 0.0 = collapsed, 1.0 = expanded
                let new_target = if spring.target > 0.5 { 0.0 } else { 1.0 };
                spring.set_target(new_target);
            }
            Message::DismissError => {
                self.stream_error = None;
                self.error_expanded = false;
            }
            Message::ToggleErrorExpand => {
                self.error_expanded = !self.error_expanded;
            }
            Message::CopyToClipboard(text) => {
                // Copy text to clipboard using arboard
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(text);
                }
            }
        }
        Task::none()
    }

    /// Helper function to get tool icons
    fn get_tool_icon(&self, name: &str) -> &'static str {
        match name.to_lowercase().as_str() {
            "execute_bash" => "○",
            "read_file" => "○",
            "write_file" | "edit_file" => "□",
            "list_directory" => "◇",
            "search_files" => "○",
            "web_search" => "⭕",
            "mcp_call" => "◊",
            "visioneer" => "○",
            _ => "□",
        }
    }

    fn handle_ui_event(&mut self, ev: UiEvent) -> Task<Message> {
        match ev {
            UiEvent::StreamStarted(id) => {
                if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                    s.set_streaming(true);
                }
            }
            UiEvent::Token(id, delta, is_final) => {
                // Find session index for syncing editors
                let session_idx = self.sessions.iter().position(|s| s.id == id);

                if let Some(idx) = session_idx {
                    let session = &mut self.sessions[idx];
                    session.append_ai_message(delta, Utc::now().to_rfc3339());

                    // Get or create the text editor content for the AI message
                    let msg_idx = session.messages.len() - 1;
                    let key = format!("{}:{}", idx, msg_idx);

                    // Only update text editor content if it doesn't exist or if this is a final token
                    // This prevents flickering during streaming
                    if !self.message_editors.contains_key(&key) {
                        self.message_editors.insert(
                            key.clone(),
                            text_editor::Content::with_text(&session.messages[msg_idx].content),
                        );
                    } else if is_final {
                        // Only update on final token to avoid constant recreation
                        if let Some(editor_content) = self.message_editors.get_mut(&key) {
                            let text = &session.messages[msg_idx].content;
                            *editor_content = text_editor::Content::with_text(text);
                        }
                    }

                    // Update markdown cache for AI messages
                    // Parse markdown on final token or periodically during streaming
                    let should_update_md = is_final || !self.markdown_cache.contains_key(&key);
                    if should_update_md && session.messages[msg_idx].is_ai() {
                        let content = &session.messages[msg_idx].content;
                        let items: Vec<markdown::Item> = markdown::parse(content).collect();
                        self.markdown_cache.insert(key, items);
                    }

                    if is_final {
                        session.flush_ai_buffer(Utc::now().to_rfc3339());
                        session.set_streaming(false);
                        // Re-focus input when streaming completes
                        return text_input::focus(input_id());
                    }
                }
            }
            UiEvent::StreamFinished(id) => {
                if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                    // Flush any remaining AI content from the buffer
                    s.flush_ai_buffer(Utc::now().to_rfc3339());
                    s.set_streaming(false);
                }
                // Re-focus input when stream finishes
                return text_input::focus(input_id());
            }
            UiEvent::StreamErrored(id, err) => {
                eprintln!("stream error {id}: {err}");
                // Store error for display to user
                self.stream_error = Some(err);
                if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                    s.set_streaming(false);
                }
                // Re-focus input on error
                return text_input::focus(input_id());
            }
            UiEvent::Thinking(id, text) => {
                // Create a thinking/reasoning bubble to show the AI's thought process
                if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
                    eprintln!(
                        "🧠 UI: Received Thinking event for session {}: {:?}",
                        id, text
                    );
                }
                if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                    if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
                        eprintln!("🧠 UI: Calling append_thinking_message");
                    }
                    s.append_thinking_message(text, Utc::now().to_rfc3339());
                } else if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
                    eprintln!("🧠 UI: Session not found for id {}", id);
                }
            }
            UiEvent::ToolCallStart(id, tool_id, name, display_args) => {
                let icon = self.get_tool_icon(&name);
                // display_args already contains "{display_name} • {formatted_args}"
                let content = format!("{} {}", icon, display_args);

                // Cache the display_args for later use in ToolCallResult
                self.tool_args_cache.insert(id, display_args);

                if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                    // Pass tool_id so we can look up streaming bash output lines
                    s.add_tool_message(content, Utc::now().to_rfc3339(), Some(tool_id));
                }
            }
            UiEvent::ToolCallResult(id, name, success, result_summary) => {
                let icon = self.get_tool_icon(&name);

                // Get cached display_args if available (contains formatted args like filename)
                let display_detail = self.tool_args_cache.remove(&id).unwrap_or_default();

                let display_name = match name.to_lowercase().as_str() {
                    "execute_bash" => "Shell",
                    "read_file" => "Read",
                    "write_file" => "Write",
                    "edit_file" => "Edit",
                    "list_directory" => "List",
                    "search_files" => "Search",
                    "web_search" => "Web",
                    "mcp_call" => "MCP",
                    "visioneer" => "Vision",
                    _ => &name,
                };

                let status = if success { "✓" } else { "✗" };

                // If we have cached display args, extract just the args part (after the •)
                let extra_info = if !display_detail.is_empty() {
                    // display_detail is like "Read • path: \"arula_core/Cargo.toml\""
                    if let Some(args_part) = display_detail.split(" • ").nth(1) {
                        // Clean up JSON formatting: remove 'key: "' prefix and trailing '"'
                        let cleaned = if let Some(colon_pos) = args_part.find(':') {
                            // Extract value after the colon
                            let value_part = args_part[colon_pos + 1..].trim();
                            // Remove surrounding quotes if present
                            value_part.trim_matches('"').to_string()
                        } else {
                            // No colon, just use as-is
                            args_part.trim_matches('"').to_string()
                        };
                        format!(" {}", cleaned)
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                let content = format!(
                    "{} {}{} {} {}",
                    icon, display_name, extra_info, status, result_summary
                );
                
                // Find session index first, then update tool message
                let session_idx = self.sessions.iter().position(|s| s.id == id);
                
                if let Some(idx) = session_idx {
                    let session = &mut self.sessions[idx];
                    session.update_tool_message(content, Utc::now().to_rfc3339());
                    
                    // Auto-collapse the tool bubble when it completes
                    // Find the last tool message index
                    if let Some(msg_idx) = session.messages.iter().rposition(|m| m.is_tool()) {
                        let key = format!("{}:{}", idx, msg_idx);
                        let spring = self.tool_animations.entry(key).or_insert_with(|| {
                            let mut s = Spring::default();
                            s.position = 1.0; // Start expanded
                            s.target = 1.0;
                            s
                        });
                        // Set target to collapsed
                        spring.set_target(0.0);
                    }
                }
            }
            UiEvent::BashOutputLine(_session_id, tool_call_id, line, is_stderr) => {
                // Accumulate bash output lines for this tool call
                self.bash_output_lines
                    .entry(tool_call_id)
                    .or_insert_with(Vec::new)
                    .push((line, is_stderr));
            }
        }
        Task::none()
    }

    fn apply_config_changes(&mut self) {
        let selected_provider = self.config_form.provider.clone();
        if self.config.active_provider != selected_provider {
            if let Err(err) = self.config.switch_provider(&selected_provider) {
                self.config_form
                    .set_error(&format!("Failed to switch provider: {err}"));
                return;
            }
        }

        self.config.set_model(&self.config_form.model);
        self.config.set_api_url(&self.config_form.api_url);
        self.config.set_api_key(&self.config_form.api_key);

        if let Some(active) = self.config.get_active_provider_config_mut() {
            active.thinking_enabled = Some(self.config_form.thinking_enabled);
            active.web_search_enabled = Some(self.config_form.web_search_enabled);
            active.tools_enabled = Some(self.config_form.ollama_tools_enabled);
            active.streaming = Some(self.config_form.streaming_enabled);
        }

        // Save global settings
        self.config.living_background_enabled = Some(self.config_form.living_background_enabled);

        match self.config.save() {
            Ok(_) => {
                if let Err(err) = self.dispatcher.update_backend(&self.config) {
                    self.config_form
                        .set_error(&format!("Saved, but backend failed to refresh: {err}"));
                    return;
                }
                self.config_form = ConfigForm::from_config(&self.config);
                self.config_form.set_success("Settings saved successfully!");
            }
            Err(err) => {
                self.config_form
                    .set_error(&format!("Failed to save settings: {err}"));
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let stream = self.dispatcher.subscription(Message::Received);
        let ticks = time::every(Duration::from_millis(TICK_INTERVAL_MS)).map(|_| Message::Tick);
        Subscription::batch(vec![stream, ticks])
    }

    fn view(&self) -> Element<'_, Message> {
        let pal = palette();

        // Show error dialog if initialization failed
        if let Some(ref error) = self.init_error {
            return self.error_view(error, pal);
        }

        let background = Canvas::new(LivingBackground::<Message>::new(
            &self.bg_state,
            pal,
            self.bg_opacity,
        ))
        .width(Length::Fill)
        .height(Length::Fill);

        // Get current session streaming state for typing indicator
        let session = &self.sessions[self.current];
        let is_streaming = session.is_streaming;

        // Build main layer with optional typing indicator above input
        let mut main_content: Vec<Element<'_, Message>> = vec![self.chat_panel(pal)];

        // Add typing indicator above input when streaming
        if is_streaming {
            main_content.push(self.typing_indicator(pal));
        }

        main_content.push(self.input_area(pal));

        let main_layer = column(main_content)
            .width(Length::Fill)
            .height(Length::Fill);

        let progress = self.menu_state.progress();
        let overlay = if progress > 0.01 {
            self.settings_overlay(pal).into()
        } else {
            Space::new(Length::Fixed(0.0), Length::Fixed(0.0)).into()
        };

        // Error notification overlay
        let error_overlay: Element<'_, Message> = if let Some(ref error) = self.stream_error {
            // Extract a user-friendly error message for collapsed view
            let short_error = if error.contains("502 Bad Gateway") {
                "Server temporarily unavailable. Please try again.".to_string()
            } else if error.contains("500") {
                "Server error occurred. Please try again later.".to_string()
            } else if error.contains("timeout") || error.contains("Timeout") {
                "Request timed out. Please check your connection.".to_string()
            } else if error.len() > 60 && !self.error_expanded {
                format!("{}...", &error[..60])
            } else {
                error.clone()
            };

            let chevron = if self.error_expanded {
                Bootstrap::ChevronDown
            } else {
                Bootstrap::ChevronRight
            };

            // Header row (always visible, clickable to expand)
            let header_row = row![
                text(Bootstrap::ExclamationTriangleFill.to_string())
                    .size(18)
                    .font(iced_fonts::BOOTSTRAP_FONT)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(Color {
                            r: 1.0,
                            g: 0.8,
                            b: 0.2,
                            a: 1.0
                        })
                    }),
                Space::with_width(Length::Fixed(8.0)),
                text(chevron.to_string())
                    .size(12)
                    .font(iced_fonts::BOOTSTRAP_FONT)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(Color { a: 0.7, ..pal.text })
                    }),
                Space::with_width(Length::Fixed(8.0)),
                text(if self.error_expanded { "Error Details".to_string() } else { short_error })
                    .size(13)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.text)
                    }),
                Space::with_width(Length::Fill),
                button(
                    text(Bootstrap::XLg.to_string())
                        .size(14)
                        .font(iced_fonts::BOOTSTRAP_FONT)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.text)
                        })
                )
                .on_press(Message::DismissError)
                .padding([4, 8])
                .style(move |_theme, _status| button::Style {
                    background: Some(Background::Color(Color::TRANSPARENT)),
                    border: Border::default(),
                    text_color: pal.text,
                    ..Default::default()
                }),
            ]
            .align_y(iced::Alignment::Center);

            // Make header clickable
            let header_button = button(header_row)
                .on_press(Message::ToggleErrorExpand)
                .padding([12, 16])
                .width(Length::Fill)
                .style(move |_theme, _status| button::Style {
                    background: Some(Background::Color(Color::TRANSPARENT)),
                    border: Border::default(),
                    text_color: pal.text,
                    ..Default::default()
                });

            // Expanded content (full error details)
            let expanded_content: Element<'_, Message> = if self.error_expanded {
                container(
                    scrollable(
                        text(error.as_str())
                            .size(12)
                            .font(Font::MONOSPACE)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(Color {
                                    r: 1.0,
                                    g: 0.7,
                                    b: 0.7,
                                    a: 1.0
                                })
                            })
                    )
                    .height(Length::Fixed(120.0))
                )
                .padding([6, 16])
                .width(Length::Fill)
                .into()
            } else {
                Space::new(Length::Fixed(0.0), Length::Fixed(0.0)).into()
            };

            let error_container = container(
                column![header_button, expanded_content].spacing(0),
            )
            .max_width(600.0)
            .style(move |_| container::Style {
                background: Some(Background::Color(Color {
                    r: 0.2,
                    g: 0.08,
                    b: 0.08,
                    a: 0.95,
                })),
                border: Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: Color {
                        r: 0.8,
                        g: 0.3,
                        b: 0.3,
                        a: 0.8,
                    },
                },
                ..Default::default()
            });

            container(column![
                Space::with_height(Length::Fill),
                error_container,
                Space::with_height(Length::Fixed(80.0)), // Space above input
            ])
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .into()
        } else {
            Space::new(Length::Fixed(0.0), Length::Fixed(0.0)).into()
        };

        let content = stack(vec![
            background.into(),
            main_layer.into(),
            overlay,
            error_overlay,
        ]);
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn error_view(&self, error: &str, pal: PaletteColors) -> Element<'_, Message> {
        let error_text = error.to_string();
        container(
            column![
                text("Initialization Error")
                    .size(32)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.danger)
                    }),
                text(error_text)
                    .size(16)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.text)
                    }),
            ]
            .spacing(16)
            .align_x(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .style(move |_| container::Style {
            background: Some(Background::Color(pal.background)),
            ..Default::default()
        })
        .into()
    }

    fn chat_panel(&self, pal: PaletteColors) -> Element<'_, Message> {
        let session = &self.sessions[self.current];

        if session.messages.is_empty() && !session.is_streaming {
            return container(
                column![
                    text("Arula Desktop")
                        .size(60)
                        .font(Font::default())
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.accent)
                        }),
                    text("Your AI Assistant")
                        .size(18)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        }),
                ]
                .spacing(10)
                .align_x(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .into();
        }

        // Build message list
        let messages: Vec<Element<'_, Message>> = session
            .messages
            .iter()
            .enumerate()
            .map(|(idx, msg)| self.message_bubble(idx, msg, pal))
            .collect();

        // Create scrollable - always anchor to bottom to prevent scroll jumping
        // when markdown rerenders or streaming ends
        scrollable(
            column(messages)
                .spacing(16) // Tighter spacing between messages
                .padding(24),
        )
        .height(Length::Fill)
        .width(Length::Fill)
        .anchor_bottom() // Always anchor to bottom like a chat app
        .into()
    }

    /// Creates an animated typing indicator for AI responses.
    fn typing_indicator(&self, pal: PaletteColors) -> Element<'_, Message> {
        // Calculate animation phase based on tick for smooth animation
        let time = self.bg_state.tick;
        let dot_size = 8.0;
        let spacing = 6.0;

        // Create 3 pulsing dots with phase offset
        let dots = row![
            container(Space::new(Length::Fixed(dot_size), Length::Fixed(dot_size))).style(
                move |_| {
                    let phase = (time * 3.0).sin() * 0.5 + 0.5;
                    container::Style {
                        background: Some(Background::Color(Color {
                            a: 0.3 + phase * 0.7,
                            ..pal.accent
                        })),
                        border: Border {
                            radius: (dot_size / 2.0).into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }
            ),
            container(Space::new(Length::Fixed(dot_size), Length::Fixed(dot_size))).style(
                move |_| {
                    let phase = ((time * 3.0) - 0.5).sin() * 0.5 + 0.5;
                    container::Style {
                        background: Some(Background::Color(Color {
                            a: 0.3 + phase * 0.7,
                            ..pal.accent
                        })),
                        border: Border {
                            radius: (dot_size / 2.0).into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }
            ),
            container(Space::new(Length::Fixed(dot_size), Length::Fixed(dot_size))).style(
                move |_| {
                    let phase = ((time * 3.0) - 1.0).sin() * 0.5 + 0.5;
                    container::Style {
                        background: Some(Background::Color(Color {
                            a: 0.3 + phase * 0.7,
                            ..pal.accent
                        })),
                        border: Border {
                            radius: (dot_size / 2.0).into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }
            ),
        ]
        .spacing(spacing)
        .align_y(iced::Alignment::Center);

        // Wrap in a bubble-like container
        let indicator_content = container(
            row![
                text(Bootstrap::Robot.to_string())
                    .font(iced_fonts::BOOTSTRAP_FONT)
                    .size(14)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.accent)
                    }),
                Space::with_width(Length::Fixed(12.0)),
                dots,
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([12, 18])
        .style(move |_| container::Style {
            background: Some(Background::Color(Color {
                a: 0.08,
                ..pal.accent
            })),
            border: Border {
                radius: 16.0.into(),
                width: 1.0,
                color: Color {
                    a: 0.15,
                    ..pal.accent
                },
            },
            ..Default::default()
        });

        // Wrap in outer container with horizontal padding to align with messages
        container(row![indicator_content])
            .padding([0, 24]) // Match chat panel horizontal padding
            .into()
    }

    fn message_bubble<'a>(
        &'a self,
        msg_idx: usize,
        message: &'a MessageEntry,
        pal: PaletteColors,
    ) -> Element<'a, Message> {
        let is_user = message.is_user();
        let is_tool = message.is_tool();
        let is_thinking = message.is_thinking();
        let key = format!("{}:{}", self.current, msg_idx);

        // Determine if this specific message is currently streaming
        let session = &self.sessions[self.current];
        let is_last_message = msg_idx == session.messages.len() - 1;
        let is_streaming = !is_user && is_last_message && session.is_streaming;

        // Calculate fade-in opacity based on time since added
        // 500ms fade duration
        let elapsed = message.added_at.elapsed().as_secs_f32();
        let fade_duration = 0.5;
        let fade_opacity = (elapsed / fade_duration).min(1.0);

        // Simplified opacity for streaming - just one consistent factor
        let streaming_opacity = if is_streaming { 0.85 } else { 1.0 };
        // Tool and thinking messages are slightly transparent/different
        let special_opacity_mult = if is_tool || is_thinking { 0.8 } else { 1.0 };

        let final_bg_multiplier = fade_opacity * special_opacity_mult * streaming_opacity;
        let final_text_multiplier = fade_opacity * special_opacity_mult * streaming_opacity;

        // Get or create the text editor content
        let content = self.message_editors.get(&key);

        // For AI messages (not user, tool, or thinking), use markdown rendering
        let is_ai_message = !is_user && !is_tool && !is_thinking;

        // Clone key for closure use (to avoid borrow after move)
        let key_for_editor = key.clone();

        let content_widget: Element<'_, Message> = if is_ai_message {
            // Use markdown rendering for AI messages
            // Get cached markdown items or parse fresh
            let md_items = self.markdown_cache.get(&key);

            if let Some(items) = md_items {
                // Render cached markdown
                markdown::view(
                    items,
                    markdown::Settings::default(),
                    markdown::Style::from_palette(iced::Theme::TokyoNightStorm.palette()),
                )
                .map(Message::LinkClicked)
                .into()
            } else {
                // Fallback to simple text while cache is being built
                // (The cache should be updated in handle_ui_event)
                text(&message.content)
                    .size(16)
                    .line_height(1.5)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(Color {
                            a: final_text_multiplier,
                            ..pal.text
                        }),
                    })
                    .into()
            }
        } else if let Some(editor_content) = content {
            // Use text_editor for selectable text (user, tool, thinking)
            let mut text_color = if is_tool {
                pal.muted
            } else if is_thinking {
                Color {
                    r: 0.7,
                    g: 0.7,
                    b: 0.9,
                    a: 1.0,
                } // Slightly purple for thinking
            } else {
                pal.text
            };
            text_color.a = final_text_multiplier;

            text_editor(editor_content)
                .on_action(move |action| {
                    Message::MessageEditorAction(key_for_editor.clone(), action)
                })
                .font(if is_tool {
                    Font::MONOSPACE
                } else {
                    Font::default()
                })
                .style(move |_theme, _status| text_editor::Style {
                    background: Background::Color(Color::TRANSPARENT),
                    border: Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: 0.0.into(),
                    },
                    icon: text_color,
                    placeholder: pal.muted,
                    value: text_color,
                    selection: Color {
                        a: 0.3,
                        ..pal.accent
                    },
                })
                .height(Length::Shrink)
                .into()
        } else {
            // Fallback to regular text if Content not yet created
            text(&message.content)
                .size(if is_tool || is_thinking { 14 } else { 16 })
                .font(if is_tool {
                    Font::MONOSPACE
                } else {
                    Font::default()
                })
                .line_height(1.5)
                .style(move |_| iced::widget::text::Style {
                    color: Some(Color {
                        a: final_text_multiplier,
                        ..if is_tool {
                            pal.muted
                        } else if is_thinking {
                            Color {
                                r: 0.7,
                                g: 0.7,
                                b: 0.9,
                                a: 1.0,
                            }
                        } else {
                            pal.text
                        }
                    }),
                })
                .into()
        };

        let timestamp =
            text(message.relative_time())
                .size(10)
                .style(move |_| iced::widget::text::Style {
                    color: Some(Color {
                        a: fade_opacity,
                        ..pal.muted
                    }), // Also fade timestamp
                });

        // Copy button for the message content
        let content_to_copy = message.content.clone();
        let copy_button = button(
            text(Bootstrap::Clipboard.to_string())
                .size(12)
                .font(iced_fonts::BOOTSTRAP_FONT)
                .style(move |_| iced::widget::text::Style {
                    color: Some(Color { a: fade_opacity * 0.6, ..pal.muted })
                })
        )
        .on_press(Message::CopyToClipboard(content_to_copy))
        .padding([2, 4])
        .style(move |_theme, status| {
            let hover_opacity = if matches!(status, button::Status::Hovered) { 1.0 } else { 0.6 };
            button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                border: Border::default(),
                text_color: Color { a: fade_opacity * hover_opacity, ..pal.muted },
                ..Default::default()
            }
        });

        // Bottom row with timestamp and copy button
        let bottom_row = row![timestamp, Space::with_width(Length::Fill), copy_button]
            .align_y(iced::Alignment::Center);

        let bubble = container(column![content_widget, bottom_row].spacing(6))
            .padding(16)
            .max_width(MESSAGE_MAX_WIDTH);

        // Custom style closure that applies the dynamic opacity
        let dynamic_style = move |base_style: container::Style| container::Style {
            background: base_style.background.map(|bg| match bg {
                Background::Color(c) => Background::Color(Color {
                    a: c.a * final_bg_multiplier,
                    ..c
                }),
                _ => bg,
            }),
            text_color: base_style.text_color.map(|c| Color {
                a: c.a * final_text_multiplier,
                ..c
            }),
            border: Border {
                color: Color {
                    a: base_style.border.color.a * fade_opacity,
                    ..base_style.border.color
                },
                ..base_style.border
            },
            ..base_style
        };

        if is_user {
            let base_style_fn = user_bubble_style(pal);
            // Apply dynamic modification to the user style
            let styled_bubble = bubble.style(move |t| dynamic_style(base_style_fn(t)));
            row![Space::with_width(Length::Fill), styled_bubble].into()
        } else if is_tool {
            // Terminal-style tool bubble with collapsible content
            return self.terminal_style_tool_bubble(msg_idx, message, &key, pal, fade_opacity);
        } else if is_thinking {
            // Thinking bubble - collapsible when finalized with "Thought for X seconds"
            return self.thinking_style_bubble(msg_idx, message, &key, pal, fade_opacity);
        } else {
            let base_style_fn = ai_bubble_style(pal, false); // Pass false since we handle opacity manually here
            let styled_bubble = bubble.style(move |t| dynamic_style(base_style_fn(t)));
            row![styled_bubble, Space::with_width(Length::Fill)].into()
        }
    }

    /// Creates a terminal-style collapsible tool bubble
    fn terminal_style_tool_bubble<'a>(
        &'a self,
        _msg_idx: usize,
        message: &'a MessageEntry,
        key: &str,
        pal: PaletteColors,
        fade_opacity: f32,
    ) -> Element<'a, Message> {
        // Get animation state: default to expanded (position=1.0)
        let spring = self.tool_animations.get(key);
        let expand_progress = spring.map(|s| s.position).unwrap_or(1.0);
        let is_collapsed = spring.map(|s| s.target < 0.5).unwrap_or(false);
        let key_owned = key.to_string();

        // Parse tool content - format varies:
        // ToolCallStart: "○ Shell • command: \"pwd\""
        // ToolCallResult: "○ Shell pwd ✓ /home/user"
        // Other tools: "○ Read • path: \"file.txt\" ✓ 732 chars"
        let content = &message.content;

        // Detect tool type from content
        #[derive(Clone, Copy, PartialEq)]
        enum ToolType {
            Shell,
            ReadFile,
            WriteFile,
            EditFile,
            ListDirectory,
            Search,
            WebSearch,
            Mcp,
            Vision,
            Other,
        }
        // Detect tool type from content
        // Content format: "{icon} {ToolName}{extra_info} {status} {result}"
        // e.g.: "◇ List ✓ 24 items" or "○ Shell ✓ exit 0"
        // Use starts_with after the icon to avoid false matches with file content
        let tool_type = if content.starts_with("○ Shell") || content.starts_with("◆ Shell") || content.contains("execute_bash") {
            ToolType::Shell
        } else if content.starts_with("□ Edit") || content.starts_with("◆ Edit") || content.contains("edit_file") {
            ToolType::EditFile
        } else if content.starts_with("□ Write") || content.starts_with("◆ Write") || content.contains("write_file") {
            ToolType::WriteFile
        } else if content.starts_with("○ Read") || content.starts_with("◆ Read") || content.contains("read_file") {
            ToolType::ReadFile
        } else if content.starts_with("◇ List") || content.starts_with("◆ List") || content.contains("list_directory") {
            ToolType::ListDirectory
        } else if content.starts_with("○ Search") || content.starts_with("◆ Search") || (content.contains("Search") && !content.contains("Web")) {
            ToolType::Search
        } else if content.starts_with("⭕ Web") || content.starts_with("◆ Web") || content.contains("web_search") {
            ToolType::WebSearch
        } else if content.starts_with("◊ MCP") || content.starts_with("◆ MCP") || content.contains("mcp_call") {
            ToolType::Mcp
        } else if content.starts_with("○ Vision") || content.starts_with("◆ Vision") || content.contains("visioneer") {
            ToolType::Vision
        } else {
            ToolType::Other
        };

        let is_shell = tool_type == ToolType::Shell;
        let is_edit = tool_type == ToolType::EditFile;
        let is_read = tool_type == ToolType::ReadFile;
        let _is_list = tool_type == ToolType::ListDirectory;

        // Check if operation completed (has ✓ or ✗)
        let has_checkmark = content.contains('✓');
        let has_error = content.contains('✗');

        // Tool-specific theming: (accent_color, header_bg, content_bg, icon, label)
        let (bubble_accent_color, header_bg_color, terminal_bg_color, tool_icon, header_label) = match tool_type {
            ToolType::Shell => (
                Color { r: 0.6, g: 0.5, b: 0.9, a: fade_opacity }, // Purple
                Color { r: 0.12, g: 0.10, b: 0.18, a: fade_opacity * 0.98 },
                Color { r: 0.06, g: 0.05, b: 0.10, a: fade_opacity * 0.98 },
                Bootstrap::Terminal,
                "Terminal",
            ),
            ToolType::ReadFile => (
                Color { r: 0.4, g: 0.7, b: 1.0, a: fade_opacity }, // Blue
                Color { r: 0.08, g: 0.12, b: 0.18, a: fade_opacity * 0.98 },
                Color { r: 0.04, g: 0.08, b: 0.12, a: fade_opacity * 0.98 },
                Bootstrap::FileEarmarkText,
                "Read File",
            ),
            ToolType::WriteFile => (
                Color { r: 0.4, g: 0.9, b: 0.5, a: fade_opacity }, // Green
                Color { r: 0.08, g: 0.15, b: 0.10, a: fade_opacity * 0.98 },
                Color { r: 0.04, g: 0.10, b: 0.06, a: fade_opacity * 0.98 },
                Bootstrap::FileEarmarkPlus,
                "Write File",
            ),
            ToolType::EditFile => (
                Color { r: 1.0, g: 0.7, b: 0.3, a: fade_opacity }, // Orange
                Color { r: 0.18, g: 0.12, b: 0.06, a: fade_opacity * 0.98 },
                Color { r: 0.12, g: 0.08, b: 0.04, a: fade_opacity * 0.98 },
                Bootstrap::FileEarmarkDiff,
                "Edit File",
            ),
            ToolType::ListDirectory => (
                Color { r: 0.3, g: 0.8, b: 0.8, a: fade_opacity }, // Teal
                Color { r: 0.06, g: 0.14, b: 0.14, a: fade_opacity * 0.98 },
                Color { r: 0.04, g: 0.10, b: 0.10, a: fade_opacity * 0.98 },
                Bootstrap::FolderFill,
                "List Directory",
            ),
            ToolType::Search => (
                Color { r: 0.5, g: 0.8, b: 1.0, a: fade_opacity }, // Cyan
                Color { r: 0.08, g: 0.12, b: 0.16, a: fade_opacity * 0.98 },
                Color { r: 0.04, g: 0.08, b: 0.12, a: fade_opacity * 0.98 },
                Bootstrap::Search,
                "Search",
            ),
            ToolType::WebSearch => (
                Color { r: 0.4, g: 0.6, b: 1.0, a: fade_opacity }, // Deep Blue
                Color { r: 0.06, g: 0.08, b: 0.16, a: fade_opacity * 0.98 },
                Color { r: 0.04, g: 0.06, b: 0.12, a: fade_opacity * 0.98 },
                Bootstrap::Globe,
                "Web Search",
            ),
            ToolType::Mcp => (
                Color { r: 0.8, g: 0.5, b: 0.9, a: fade_opacity }, // Magenta
                Color { r: 0.14, g: 0.08, b: 0.16, a: fade_opacity * 0.98 },
                Color { r: 0.10, g: 0.05, b: 0.12, a: fade_opacity * 0.98 },
                Bootstrap::PlugFill,
                "MCP Tool",
            ),
            ToolType::Vision => (
                Color { r: 0.9, g: 0.6, b: 0.8, a: fade_opacity }, // Pink
                Color { r: 0.16, g: 0.10, b: 0.14, a: fade_opacity * 0.98 },
                Color { r: 0.12, g: 0.06, b: 0.10, a: fade_opacity * 0.98 },
                Bootstrap::EyeFill,
                "Vision",
            ),
            ToolType::Other => (
                Color { r: 0.6, g: 0.6, b: 0.6, a: fade_opacity }, // Gray
                Color { r: 0.12, g: 0.12, b: 0.12, a: fade_opacity * 0.98 },
                Color { r: 0.08, g: 0.08, b: 0.08, a: fade_opacity * 0.98 },
                Bootstrap::GearFill,
                "Tool",
            ),
        };

        // Extract command/argument - improved parsing
        let command_display = {
            let tool_names = [
                "Shell", "Read", "Write", "Edit", "List", "Search", "Web", "MCP", "Vision",
            ];
            let mut after_tool = "";

            for name in tool_names {
                if let Some(idx) = content.find(name) {
                    after_tool = &content[idx + name.len()..];
                    break;
                }
            }

            let cleaned = after_tool
                .trim_start()
                .trim_start_matches('•')
                .trim_start()
                .trim_start_matches("command:")
                .trim_start_matches("path:")
                .trim_start_matches("query:")
                .trim_start()
                .trim_matches('"');

            let before_status = cleaned
                .split('✓')
                .next()
                .and_then(|s| Some(s.split('✗').next().unwrap_or(s)))
                .unwrap_or(cleaned)
                .trim()
                .trim_end_matches('"');

            if before_status.is_empty() {
                "...".to_string()
            } else if before_status.len() > 50 {
                format!("{}...", &before_status[..47])
            } else {
                before_status.to_string()
            }
        };

        // Extract result text (everything after ✓ or ✗)
        let result_text = if has_checkmark {
            content.split('✓').nth(1).map(|s| s.trim().to_string())
        } else if has_error {
            content.split('✗').nth(1).map(|s| s.trim().to_string())
        } else {
            None
        };

        // Status icon using Bootstrap icons
        let status_icon = if has_checkmark {
            Bootstrap::CheckLg
        } else if has_error {
            Bootstrap::XLg
        } else {
            Bootstrap::Circle
        };

        let status_color = if has_checkmark {
            Color { r: 0.4, g: 1.0, b: 0.5, a: fade_opacity }
        } else if has_error {
            Color { r: 1.0, g: 0.4, b: 0.4, a: fade_opacity }
        } else {
            bubble_accent_color
        };

        // Dropdown chevron using Bootstrap
        let chevron_icon = if is_collapsed {
            Bootstrap::ChevronRight
        } else {
            Bootstrap::ChevronDown
        };

        // For read file, include the result summary in the header display
        let header_detail = if is_read {
            // Show full info: "filename • X chars"
            if let Some(ref result) = result_text {
                format!("{} • {}", command_display, result)
            } else {
                command_display.clone()
            }
        } else {
            command_display.clone()
        };

        // Build header row - chevron goes on the RIGHT side (after status icon)
        // ReadFile doesn't get a chevron since it's not expandable
        let mut header_row = row![
            text(tool_icon.to_string())
                .size(14)
                .font(iced_fonts::BOOTSTRAP_FONT)
                .style(move |_| iced::widget::text::Style {
                    color: Some(bubble_accent_color)
                }),
            Space::with_width(Length::Fixed(6.0)),
            text(header_label)
                .size(13)
                .style(move |_| iced::widget::text::Style {
                    color: Some(bubble_accent_color)
                }),
            Space::with_width(Length::Fixed(8.0)),
            text(header_detail)
                .size(12)
                .font(Font::MONOSPACE)
                .style(move |_| iced::widget::text::Style {
                    color: Some(Color { a: fade_opacity * 0.7, ..pal.text })
                }),
            Space::with_width(Length::Fill),
            text(status_icon.to_string())
                .size(14)
                .font(iced_fonts::BOOTSTRAP_FONT)
                .style(move |_| iced::widget::text::Style {
                    color: Some(status_color)
                }),
        ]
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

        // Add chevron on right side for expandable tools (not read file)
        if !is_read {
            header_row = header_row.push(Space::with_width(Length::Fixed(8.0)));
            header_row = header_row.push(
                text(chevron_icon.to_string())
                    .size(12)
                    .font(iced_fonts::BOOTSTRAP_FONT)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(Color { a: fade_opacity * 0.7, ..pal.text })
                    }),
            );
        }

        // Header button - only clickable for non-read tools
        let header = if is_read {
            // Read file: non-expandable, just a container with full rounded corners
            button(header_row)
                .padding([8, 12])
                .style(move |_theme, _status| button::Style {
                    background: Some(Background::Color(header_bg_color)),
                    border: Border {
                        radius: 8.0.into(),
                        width: 1.0,
                        color: Color {
                            a: bubble_accent_color.a * 0.5,
                            ..bubble_accent_color
                        },
                    },
                    text_color: pal.text,
                    ..Default::default()
                })
        } else {
            // Other tools: expandable with toggle
            button(header_row)
                .on_press(Message::ToggleToolCollapse(key_owned))
                .padding([8, 12])
                .style(move |_theme, _status| button::Style {
                    background: Some(Background::Color(header_bg_color)),
                    border: Border {
                        radius: if is_collapsed { 8.0.into() } else { iced::border::Radius::new(0.0).top(8.0) },
                        width: 1.0,
                        color: Color {
                            a: bubble_accent_color.a * 0.5,
                            ..bubble_accent_color
                        },
                    },
                    text_color: pal.text,
                    ..Default::default()
                })
        };

        // Terminal content area (only shown when not collapsed, and never for read file)
        // Uses expand_progress for smooth animation (0.0 = collapsed, 1.0 = expanded)
        let terminal_content: Element<'_, Message> = if is_read || expand_progress < 0.01 {
            Space::new(Length::Fixed(0.0), Length::Fixed(0.0)).into()
        } else {
            // Black terminal background with command prompt
            let prompt_prefix = if is_shell { "$ " } else { "> " };

            // Calculate content opacity for animation (fade in/out with expand)
            let content_opacity = fade_opacity * expand_progress;

            // Terminal text colors - neutral prompt, white command
            let prompt_color = Color {
                r: 0.6,
                g: 0.6,
                b: 0.65,
                a: content_opacity,
            }; // Light gray prompt
            let command_color = Color {
                r: 0.9,
                g: 0.9,
                b: 0.9,
                a: content_opacity,
            }; // White command

            // Result color with glow effect
            let result_glow_color = if has_checkmark {
                Color {
                    r: 0.4,
                    g: 1.0,
                    b: 0.5,
                    a: content_opacity,
                } // Bright green glow
            } else if has_error {
                Color {
                    r: 1.0,
                    g: 0.4,
                    b: 0.4,
                    a: content_opacity,
                } // Bright red glow
            } else {
                Color {
                    r: 0.7,
                    g: 0.7,
                    b: 0.7,
                    a: content_opacity,
                } // Gray
            };

            // Command line with green prompt and white command
            let command_row = row![
                text(prompt_prefix)
                    .size(13)
                    .font(Font::MONOSPACE)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(prompt_color)
                    }),
                text(command_display.clone())
                    .size(13)
                    .font(Font::MONOSPACE)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(command_color)
                    })
            ];

            let mut terminal_column = column![command_row].spacing(4);

            // Check for streaming bash output lines first (keyed by tool_call_id)
            let streaming_lines: Option<&Vec<(String, bool)>> = message
                .tool_call_id
                .as_ref()
                .and_then(|id| self.bash_output_lines.get(id));

            if let Some(lines) = streaming_lines {
                // Use streaming lines - show each with proper color
                // stdout = green, stderr = orange/red
                for (line, is_stderr) in lines.iter() {
                    let line_color = if *is_stderr {
                        Color {
                            r: 1.0,
                            g: 0.6,
                            b: 0.4,
                            a: content_opacity,
                        } // Orange for stderr
                    } else {
                        result_glow_color
                    };

                    terminal_column = terminal_column.push(
                        text(line.clone())
                            .size(12)
                            .font(Font::MONOSPACE)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(line_color),
                            }),
                    );
                }
            } else if let Some(ref result) = result_text {
                // Fallback to result text if no streaming lines
                if !result.is_empty() {
                    // Handle Edit file specially - show diff with colors
                    if is_edit {
                        for line in result.lines() {
                            let line_color = if line.starts_with('+') || line.starts_with("+ ") {
                                Color { r: 0.4, g: 1.0, b: 0.5, a: content_opacity } // Green for additions
                            } else if line.starts_with('-') || line.starts_with("- ") {
                                Color { r: 1.0, g: 0.4, b: 0.4, a: content_opacity } // Red for deletions
                            } else if line.starts_with("@@") || line.contains("line") {
                                Color { r: 0.6, g: 0.8, b: 1.0, a: content_opacity } // Blue for line markers
                            } else {
                                Color { r: 0.7, g: 0.7, b: 0.7, a: content_opacity } // Gray for context
                            };

                            terminal_column = terminal_column.push(
                                text(line.to_string())
                                    .size(12)
                                    .font(Font::MONOSPACE)
                                    .style(move |_| iced::widget::text::Style {
                                        color: Some(line_color),
                                    }),
                            );
                        }
                    } else {
                        // Default rendering for other tools
                        for line in result.lines() {
                            terminal_column = terminal_column.push(
                                text(line.to_string()).size(12).font(Font::MONOSPACE).style(
                                    move |_| iced::widget::text::Style {
                                        color: Some(result_glow_color),
                                    },
                                ),
                            );
                        }
                    }
                }
            }

            // Wrap in container with tool-themed background
            // Animate background opacity
            let animated_bg = Color { a: terminal_bg_color.a * expand_progress, ..terminal_bg_color };
            let terminal_inner = container(terminal_column)
                .padding([10, 14])
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(animated_bg)),
                    border: Border {
                        radius: iced::border::Radius::new(0.0).bottom(8.0),
                        width: 1.0,
                        color: Color { a: bubble_accent_color.a * 0.3 * expand_progress, ..bubble_accent_color },
                    },
                    ..Default::default()
                });

            // Calculate line count to determine base height
            let line_count = if let Some(lines) = streaming_lines {
                lines.len()
            } else if let Some(ref result) = result_text {
                result.lines().count()
            } else {
                0
            };

            // Calculate actual content height based on line count
            // Each line ~18px (font size 12 + line spacing), plus padding
            let line_height = 18.0_f32;
            let content_padding = 24.0_f32; // Top + bottom padding
            let natural_height = (line_count as f32 * line_height + content_padding).max(40.0);
            
            // Cap at max scrollable height for tall content
            let max_visible_height = 200.0_f32;
            let base_height = natural_height.min(max_visible_height);
            let animated_height = base_height * expand_progress;
            
            let scroll_height = if expand_progress < 0.99 {
                // During animation, use animated height scaling to actual size
                Length::Fixed(animated_height.max(1.0))
            } else if natural_height > max_visible_height {
                // Tall content: use scrollable with max height
                Length::Fixed(max_visible_height)
            } else {
                // Short content: use natural size
                Length::Shrink
            };

            // Wrap in scrollable
            scrollable(terminal_inner)
                .height(scroll_height)
                .width(Length::Fill)
                .into()
        };

        // Timestamp
        let timestamp =
            text(message.relative_time())
                .size(10)
                .style(move |_| iced::widget::text::Style {
                    color: Some(Color {
                        a: fade_opacity * 0.7,
                        ..pal.muted
                    }),
                });

        // Outer container with border
        let bubble = container(
            column![
                header,
                terminal_content,
                container(timestamp).padding([4, 12]),
            ]
            .spacing(0),
        )
        .max_width(MESSAGE_MAX_WIDTH)
        .style(move |_| container::Style {
            background: None,
            border: Border {
                radius: 8.0.into(),
                width: 1.0,
                color: Color {
                    a: bubble_accent_color.a * 0.5,
                    ..bubble_accent_color
                },
            },
            ..Default::default()
        });

        row![bubble, Space::with_width(Length::Fill)].into()
    }

    /// Creates a collapsible thinking bubble
    /// Shows "Thought for X seconds" when finalized, or "Thinking..." when active
    fn thinking_style_bubble<'a>(
        &'a self,
        _msg_idx: usize,
        message: &'a MessageEntry,
        key: &str,
        pal: PaletteColors,
        fade_opacity: f32,
    ) -> Element<'a, Message> {
        let is_finalized = message.thinking_duration_secs.is_some();
        // Get animation state: default to collapsed (0.0) for finalized thinking
        let spring = self.tool_animations.get(key);
        let expand_progress = spring.map(|s| s.position).unwrap_or(if is_finalized { 0.0 } else { 1.0 });
        let is_collapsed = spring.map(|s| s.target < 0.5).unwrap_or(is_finalized);
        let key_owned = key.to_string();

        // Purple/blue color scheme for thinking
        let accent_color = Color {
            r: 0.5,
            g: 0.5,
            b: 0.9,
            a: fade_opacity,
        };
        let header_bg = Color {
            r: 0.10,
            g: 0.10,
            b: 0.18,
            a: fade_opacity * 0.98,
        };
        let content_bg = Color {
            r: 0.06,
            g: 0.06,
            b: 0.12,
            a: fade_opacity * 0.98,
        };

        // Header text based on state
        let header_text = if let Some(duration) = message.thinking_duration_secs {
            format!("Thought for {}s", duration.round() as i32)
        } else {
            "Thinking...".to_string()
        };

        // Build header row
        let mut header_row = row![
            text(Bootstrap::Lightbulb.to_string())
                .size(14)
                .font(iced_fonts::BOOTSTRAP_FONT)
                .style(move |_| iced::widget::text::Style {
                    color: Some(accent_color)
                }),
            Space::with_width(Length::Fixed(8.0)),
            text(header_text)
                .size(13)
                .style(move |_| iced::widget::text::Style {
                    color: Some(accent_color)
                }),
            Space::with_width(Length::Fill),
        ]
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

        // Add chevron for finalized thinking (expandable)
        if is_finalized {
            let chevron = if is_collapsed {
                Bootstrap::ChevronRight
            } else {
                Bootstrap::ChevronDown
            };
            header_row = header_row.push(
                text(chevron.to_string())
                    .size(12)
                    .font(iced_fonts::BOOTSTRAP_FONT)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(Color { a: fade_opacity * 0.7, ..pal.text })
                    }),
            );
        }

        // Header button
        let header = if is_finalized {
            button(header_row)
                .on_press(Message::ToggleToolCollapse(key_owned))
                .padding([8, 12])
                .style(move |_theme, _status| button::Style {
                    background: Some(Background::Color(header_bg)),
                    border: Border {
                        radius: if is_collapsed { 8.0.into() } else { iced::border::Radius::new(0.0).top(8.0) },
                        width: 1.0,
                        color: Color { a: accent_color.a * 0.5, ..accent_color },
                    },
                    text_color: pal.text,
                    ..Default::default()
                })
        } else {
            // Active thinking - not clickable
            button(header_row)
                .padding([8, 12])
                .style(move |_theme, _status| button::Style {
                    background: Some(Background::Color(header_bg)),
                    border: Border {
                        radius: 8.0.into(),
                        width: 1.0,
                        color: Color { a: accent_color.a * 0.5, ..accent_color },
                    },
                    text_color: pal.text,
                    ..Default::default()
                })
        };

        // Content area (only for expanded finalized thinking)
        // Uses expand_progress for smooth animation
        let thinking_content: Element<'_, Message> = if !is_finalized || expand_progress < 0.01 {
            Space::new(Length::Fixed(0.0), Length::Fixed(0.0)).into()
        } else {
            // Animate opacity with expand
            let content_opacity = fade_opacity * expand_progress;
            let text_color = Color {
                r: 0.7,
                g: 0.7,
                b: 0.9,
                a: content_opacity,
            };

            // Animate background opacity
            let animated_bg = Color { a: content_bg.a * expand_progress, ..content_bg };
            let content_inner = container(
                text(&message.content)
                    .size(13)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(text_color),
                    }),
            )
            .padding([10, 14])
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(animated_bg)),
                border: Border {
                    radius: iced::border::Radius::new(0.0).bottom(8.0),
                    width: 1.0,
                    color: Color { a: accent_color.a * 0.3 * expand_progress, ..accent_color },
                },
                ..Default::default()
            });

            // Calculate actual content height based on line count
            let line_count = message.content.lines().count();
            let line_height = 18.0_f32;
            let content_padding = 24.0_f32;
            let natural_height = (line_count as f32 * line_height + content_padding).max(40.0);
            
            let max_visible_height = 200.0_f32;
            let base_height = natural_height.min(max_visible_height);
            let animated_height = base_height * expand_progress;
            
            let scroll_height = if expand_progress < 0.99 {
                Length::Fixed(animated_height.max(1.0))
            } else if natural_height > max_visible_height {
                Length::Fixed(max_visible_height)
            } else {
                Length::Shrink
            };

            scrollable(content_inner)
                .height(scroll_height)
                .width(Length::Fill)
                .into()
        };

        // Outer container
        let bubble = container(
            column![header, thinking_content].spacing(0),
        )
        .max_width(MESSAGE_MAX_WIDTH);

        row![bubble, Space::with_width(Length::Fill)].into()
    }

    fn input_area(&self, pal: PaletteColors) -> Element<'_, Message> {
        // When menu overlay is visible (animating or fully open), show a placeholder space
        // instead of the button to avoid overlapping with the floating close button in the overlay
        // Use progress instead of is_open to prevent flickering during close animation
        let overlay_visible = self.menu_state.progress() > 0.01;
        let menu_button_or_space: Element<'_, Message> = if overlay_visible {
            Space::new(
                Length::Fixed(MENU_BUTTON_SIZE),
                Length::Fixed(MENU_BUTTON_SIZE),
            )
            .into()
        } else {
            self.menu_button(pal)
        };

        // Check if current session is streaming
        let is_streaming = self
            .sessions
            .get(self.current)
            .map(|s| s.is_streaming)
            .unwrap_or(false);

        let input_field = text_input("Type your message...", &self.draft)
            .id(input_id())
            .on_input(Message::DraftChanged)
            .on_submit(Message::SendPrompt)
            .padding(14)
            .style(chat_input_style(pal))
            .width(Length::Fill);

        // Show Send or Stop button based on streaming state
        let action_btn: Element<'_, Message> = if is_streaming {
            // Stop button with danger color
            button(text("Stop").size(14))
                .on_press(Message::StopStream)
                .padding([14, 24])
                .style(move |_theme, status| {
                    let bg_alpha = match status {
                        iced::widget::button::Status::Hovered => 1.0,
                        iced::widget::button::Status::Pressed => 0.8,
                        _ => 0.9,
                    };
                    iced::widget::button::Style {
                        background: Some(Background::Color(Color {
                            a: bg_alpha,
                            ..pal.danger
                        })),
                        border: Border {
                            radius: 8.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        text_color: pal.text,
                        ..Default::default()
                    }
                })
                .into()
        } else {
            // Normal send button
            button(text("Send").size(14))
                .on_press(Message::SendPrompt)
                .padding([14, 24])
                .style(send_button_style(pal))
                .into()
        };

        let input_container = container(
            row![input_field, action_btn]
                .align_y(iced::Alignment::Center)
                .spacing(0),
        )
        .padding(0)
        .style(chat_input_container_style(pal))
        .width(Length::Fill);

        container(
            row![menu_button_or_space, input_container]
                .spacing(12)
                .align_y(iced::Alignment::Center),
        )
        .padding(16)
        .style(transparent_style())
        .into()
    }

    fn menu_button(&self, pal: PaletteColors) -> Element<'_, Message> {
        let is_open = self.menu_state.is_open();
        let color = if is_open { pal.text } else { pal.accent };

        // Use Bootstrap Icons - gear-fill for settings, x-lg for close
        let icon_char = if is_open {
            iced_fonts::Bootstrap::XLg
        } else {
            iced_fonts::Bootstrap::GearFill
        };

        let icon_text = text(icon_char.to_string())
            .size(22)
            .font(iced_fonts::BOOTSTRAP_FONT)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .style(move |_| iced::widget::text::Style { color: Some(color) });

        button(
            container(icon_text)
                .width(Length::Fixed(MENU_BUTTON_SIZE))
                .height(Length::Fixed(MENU_BUTTON_SIZE))
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center),
        )
        .on_press(if is_open {
            Message::CloseSettings
        } else {
            Message::ToggleSettings
        })
        .style(cog_button_container_style_button(pal, is_open))
        .padding(0)
        .into()
    }

    fn settings_overlay(&self, pal: PaletteColors) -> Element<'_, Message> {
        let progress = self.menu_state.progress();
        let form = &self.config_form;
        let settings = &self.settings_state;

        // Calculate transition animation values
        let page_progress = settings.progress();
        let is_on_submenu = settings.current_page != SettingsPage::Main;

        // Submenu slide-in offset (starts offscreen to the right, slides to position)
        let submenu_slide = if is_on_submenu {
            PAGE_SLIDE_DISTANCE * (1.0 - page_progress)
        } else {
            PAGE_SLIDE_DISTANCE
        };

        // Opacity for submenu during slide
        let submenu_opacity = if is_on_submenu { page_progress } else { 0.0 };

        // Build main menu (always shown on left)
        let main_menu = self.settings_main_page(pal, is_on_submenu);

        // Build submenu content if we're on a submenu page
        let submenu_content: Option<Element<'_, Message>> =
            if is_on_submenu || settings.is_transitioning() {
                Some(match settings.current_page {
                    SettingsPage::Main => Space::new(Length::Fixed(0.0), Length::Fixed(0.0)).into(),
                    SettingsPage::Provider => self.settings_provider_page(pal, form),
                    SettingsPage::Api => self.settings_provider_page(pal, form), // Redirect to provider
                    SettingsPage::Behavior => self.settings_behavior_page(pal, form),
                    SettingsPage::Appearance => self.settings_appearance_page(pal, form),
                    SettingsPage::ModelSelector => self.settings_model_selector_page(pal),
                })
            } else {
                None
            };

        // Create the dual-panel content layout
        let content_layout: Element<'_, Message> = if let Some(submenu) = submenu_content {
            // Submenu with slide effect using Space for offset
            let submenu_with_slide = row![
                Space::with_width(Length::Fixed(submenu_slide)),
                container(submenu).style(move |_| container::Style {
                    background: None,
                    text_color: Some(Color {
                        a: submenu_opacity,
                        ..pal.text
                    }),
                    ..Default::default()
                }),
            ];

            row![
                main_menu,
                Space::with_width(Length::Fixed(20.0)),
                submenu_with_slide,
            ]
            .align_y(iced::Alignment::Start)
            .into()
        } else {
            main_menu
        };

        let styled_content = container(content_layout)
            .padding(20)
            .align_x(Horizontal::Left)
            .align_y(Vertical::Top);

        let liquid_bg = Canvas::new(LiquidMenuBackground::<Message>::new(&self.menu_state, pal))
            .width(Length::Fill)
            .height(Length::Fill);

        // Create floating close button for the overlay (in bottom-left corner)
        let close_icon = text(iced_fonts::Bootstrap::XLg.to_string())
            .size(22)
            .font(iced_fonts::BOOTSTRAP_FONT)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .style(move |_| iced::widget::text::Style {
                color: Some(pal.text),
            });

        let floating_close_btn = button(
            container(close_icon)
                .width(Length::Fixed(MENU_BUTTON_SIZE))
                .height(Length::Fixed(MENU_BUTTON_SIZE))
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center),
        )
        .on_press(Message::CloseSettings)
        .style(cog_button_container_style_button(pal, true))
        .padding(0);

        // Position the close button in the bottom-left corner using a column with spacing
        let close_btn_positioned = column![
            Space::with_height(Length::Fill),
            row![
                container(floating_close_btn).padding(16),
                Space::with_width(Length::Fill)
            ]
        ];

        let content = if progress > 0.2 {
            styled_content.into()
        } else {
            Space::new(Length::Fixed(0.0), Length::Fixed(0.0)).into()
        };

        // Stack: background, content, and floating close button on top
        stack(vec![liquid_bg.into(), content, close_btn_positioned.into()]).into()
    }

    /// Renders the main settings page with category buttons.
    fn settings_main_page(&self, pal: PaletteColors, is_on_submenu: bool) -> Element<'_, Message> {
        // Simple compact header (no tilt card)
        let header = column![
            text("Settings")
                .size(22)
                .style(move |_| iced::widget::text::Style {
                    color: Some(pal.accent)
                }),
            text("Configure your AI")
                .size(12)
                .style(move |_| iced::widget::text::Style {
                    color: Some(pal.muted)
                })
        ]
        .spacing(2);

        // Category buttons with icons
        let provider_btn = self.category_button(
            iced_fonts::Bootstrap::Cpu,
            "Provider & Model",
            "AI provider and model",
            Message::SettingsNavigate(SettingsPage::Provider),
            pal,
        );

        let behavior_btn = self.category_button(
            iced_fonts::Bootstrap::Sliders,
            "Behavior",
            "AI behavior settings",
            Message::SettingsNavigate(SettingsPage::Behavior),
            pal,
        );

        let appearance_btn = self.category_button(
            iced_fonts::Bootstrap::Palette,
            "Appearance",
            "Visual settings",
            Message::SettingsNavigate(SettingsPage::Appearance),
            pal,
        );

        // Dim the menu slightly when a submenu is open to show focus shift
        let menu_opacity = if is_on_submenu { 0.6 } else { 1.0 };

        container(
            column![
                header,
                Space::with_height(Length::Fixed(16.0)),
                provider_btn,
                behavior_btn,
                appearance_btn,
            ]
            .spacing(6)
            .width(Length::Fixed(SETTINGS_CARD_WIDTH)),
        )
        .style(move |_| container::Style {
            background: None,
            text_color: Some(Color {
                a: menu_opacity,
                ..pal.text
            }),
            ..Default::default()
        })
        .into()
    }

    /// Creates a category button for the main settings page.
    fn category_button(
        &self,
        icon: iced_fonts::Bootstrap,
        title: &'static str,
        subtitle: &'static str,
        on_press: Message,
        pal: PaletteColors,
    ) -> Element<'static, Message> {
        let icon_text = text(icon.to_string())
            .size(20)
            .font(iced_fonts::BOOTSTRAP_FONT)
            .style(move |_| iced::widget::text::Style {
                color: Some(pal.accent),
            });

        let arrow = text(iced_fonts::Bootstrap::ChevronRight.to_string())
            .size(16)
            .font(iced_fonts::BOOTSTRAP_FONT)
            .style(move |_| iced::widget::text::Style {
                color: Some(pal.muted),
            });

        let content = row![
            icon_text,
            Space::with_width(Length::Fixed(12.0)),
            column![
                text(title)
                    .size(16)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.text)
                    }),
                text(subtitle)
                    .size(12)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    }),
            ]
            .spacing(2),
            Space::with_width(Length::Fill),
            arrow,
        ]
        .align_y(iced::Alignment::Center)
        .padding(16);

        button(content)
            .on_press(on_press)
            .width(Length::Fill)
            .style(move |_theme, status| {
                let bg_alpha = match status {
                    iced::widget::button::Status::Hovered => 0.15,
                    iced::widget::button::Status::Pressed => 0.2,
                    _ => 0.08,
                };
                iced::widget::button::Style {
                    background: Some(Background::Color(Color {
                        a: bg_alpha,
                        ..pal.accent
                    })),
                    border: Border {
                        radius: 12.0.into(),
                        width: 1.0,
                        color: Color {
                            a: 0.1,
                            ..pal.accent
                        },
                    },
                    text_color: pal.text,
                    ..Default::default()
                }
            })
            .into()
    }

    /// Renders the Provider & Model settings page.
    fn settings_provider_page<'a>(
        &'a self,
        pal: PaletteColors,
        form: &'a ConfigForm,
    ) -> Element<'a, Message> {
        let header = text("Provider & Model")
            .size(18)
            .style(move |_| iced::widget::text::Style {
                color: Some(pal.text),
            });

        let base_content =
            column![
                text("Provider")
                    .size(12)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    }),
                pick_list(
                    form.provider_options.clone(),
                    Some(form.provider.clone()),
                    Message::ConfigProviderChanged
                ),
                Space::with_height(Length::Fixed(12.0)),
                text("Model")
                    .size(12)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    }),
                button(
                    row![
                        text(&form.model)
                            .size(13)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(pal.text)
                            }),
                        Space::with_width(Length::Fill),
                        text(Bootstrap::ChevronRight.to_string())
                            .font(iced_fonts::BOOTSTRAP_FONT)
                            .size(12)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(pal.accent)
                            }),
                    ]
                    .align_y(iced::Alignment::Center)
                )
                .on_press(Message::OpenModelSelector)
                .padding([10, 14])
                .width(Length::Fill)
                .style(move |_theme, status| {
                    let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                    iced::widget::button::Style {
                        background: Some(Background::Color(Color {
                            a: if is_hovered { 0.15 } else { 0.1 },
                            ..pal.accent
                        })),
                        border: Border {
                            radius: 8.0.into(),
                            width: 1.0,
                            color: Color {
                                a: if is_hovered { 0.3 } else { 0.2 },
                                ..pal.accent
                            },
                        },
                        text_color: pal.text,
                        ..Default::default()
                    }
                }),
                Space::with_height(Length::Fixed(16.0)),
                text("API Key")
                    .size(12)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    }),
                text_input("Enter your API key", &form.api_key)
                    .secure(true)
                    .on_input(Message::ConfigApiKeyChanged)
                    .padding(8)
                    .style(input_style(pal)),
                Space::with_height(Length::Fixed(12.0)),
                text("Endpoint URL")
                    .size(12)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    }),
            ]
            .spacing(8)
            .width(Length::Fill);

        // Add endpoint UI - for z.ai show dropdown, for others show text input
        let endpoint_content: Element<'a, Message> = if form.is_zai_provider() {
            // Z.AI provider: show endpoint dropdown with predefined options
            let mut endpoint_options = form.endpoint_options.clone();
            // Add "Custom" option if not already present
            if !endpoint_options.contains(&"Custom".to_string()) {
                endpoint_options.push("Custom".to_string());
            }
            
            let endpoint_selector = pick_list(
                endpoint_options,
                Some(form.endpoint_name.clone()),
                Message::ConfigEndpointChanged
            );

            // Show text input only when Custom is selected
            if form.endpoint_name == "Custom" {
                column![
                    endpoint_selector,
                    Space::with_height(Length::Fixed(8.0)),
                    text("Custom URL")
                        .size(11)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        }),
                    text_input("https://api.z.ai/custom/endpoint", &form.api_url)
                        .on_input(Message::ConfigApiUrlChanged)
                        .padding(8)
                        .style(input_style(pal)),
                ]
                .spacing(4)
                .into()
            } else {
                // Show selected endpoint URL as read-only info
                column![
                    endpoint_selector,
                    Space::with_height(Length::Fixed(4.0)),
                    text(&form.api_url)
                        .size(11)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(Color { a: 0.6, ..pal.text })
                        }),
                ]
                .spacing(4)
                .into()
            }
        } else {
            // Other providers: show regular text input
            text_input("https://api.example.com/v1", &form.api_url)
                .on_input(Message::ConfigApiUrlChanged)
                .padding(8)
                .style(input_style(pal))
                .into()
        };

        let content = container(
            column![
                base_content,
                endpoint_content,
                Space::with_height(Length::Fill),
            ]
            .spacing(8)
            .width(Length::Fill),
        )
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(Color {
                a: 0.08,
                ..pal.accent
            })),
            border: Border {
                radius: 12.0.into(),
                width: 1.0,
                color: Color {
                    a: 0.15,
                    ..pal.accent
                },
            },
            ..Default::default()
        });

        let status_text = form.status.clone().unwrap_or_default();
        let save_btn = button("Save Changes")
            .on_press(Message::SaveConfig)
            .padding([10, 20])
            .style(primary_button_style(pal));
        let status = text(status_text)
            .size(12)
            .style(move |_| iced::widget::text::Style {
                color: Some(pal.accent),
            });

        column![
            header,
            Space::with_height(Length::Fixed(12.0)),
            content,
            Space::with_height(Length::Fixed(12.0)),
            row![save_btn, Space::with_width(Length::Fixed(12.0)), status]
                .align_y(iced::Alignment::Center),
        ]
        .spacing(4)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    /// Renders the Behavior settings page.
    fn settings_behavior_page<'a>(
        &'a self,
        pal: PaletteColors,
        form: &'a ConfigForm,
    ) -> Element<'a, Message> {
        let header = text("Behavior")
            .size(18)
            .style(move |_| iced::widget::text::Style {
                color: Some(pal.text),
            });

        let content = container(
            column![
                text("System Prompt")
                    .size(12)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    }),
                text_input("You are a helpful assistant...", &form.system_prompt)
                    .on_input(Message::ConfigSystemPromptChanged)
                    .padding(8)
                    .style(input_style(pal)),
                Space::with_height(Length::Fixed(12.0)),
                row![
                    column![
                        text(format!("Temperature: {:.1}", form.temperature))
                            .size(12)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(pal.muted)
                            }),
                        iced::widget::slider(
                            0.0..=2.0,
                            form.temperature,
                            Message::ConfigTemperatureChanged
                        )
                        .step(0.1)
                    ]
                    .width(Length::Fill),
                    column![
                        text("Max Tokens")
                            .size(12)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(pal.muted)
                            }),
                        text_input("2048", &form.max_tokens.to_string())
                            .on_input(Message::ConfigMaxTokensChanged)
                            .padding(4)
                            .style(input_style(pal))
                    ]
                    .width(Length::Fixed(80.0))
                ]
                .spacing(16),
                Space::with_height(Length::Fixed(12.0)),
                row![
                    text("Enable Streaming")
                        .size(14)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.text)
                        }),
                    Space::with_width(Length::Fill),
                    iced::widget::toggler(form.streaming_enabled)
                        .on_toggle(Message::ConfigStreamingToggled)
                        .width(Length::Shrink)
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center),
                Space::with_height(Length::Fill),
            ]
            .spacing(8)
            .width(Length::Fill),
        )
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(Color {
                a: 0.08,
                ..pal.accent
            })),
            border: Border {
                radius: 12.0.into(),
                width: 1.0,
                color: Color {
                    a: 0.15,
                    ..pal.accent
                },
            },
            ..Default::default()
        });

        let status_text = form.status.clone().unwrap_or_default();
        let save_btn = button("Save Changes")
            .on_press(Message::SaveConfig)
            .padding([10, 20])
            .style(primary_button_style(pal));
        let status = text(status_text)
            .size(12)
            .style(move |_| iced::widget::text::Style {
                color: Some(pal.accent),
            });

        column![
            header,
            Space::with_height(Length::Fixed(12.0)),
            content,
            Space::with_height(Length::Fixed(12.0)),
            row![save_btn, Space::with_width(Length::Fixed(12.0)), status]
                .align_y(iced::Alignment::Center),
        ]
        .spacing(4)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    /// Renders the Appearance settings page.
    fn settings_appearance_page<'a>(
        &'a self,
        pal: PaletteColors,
        form: &'a ConfigForm,
    ) -> Element<'a, Message> {
        let header = text("Appearance")
            .size(18)
            .style(move |_| iced::widget::text::Style {
                color: Some(pal.text),
            });

        let content = container(
            column![
                text("Visual Settings")
                    .size(14)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    }),
                Space::with_height(Length::Fixed(12.0)),
                row![
                    column![
                        text("Living Background").size(14).style(move |_| {
                            iced::widget::text::Style {
                                color: Some(pal.text),
                            }
                        }),
                        text("Animated particle background")
                            .size(12)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(pal.muted)
                            }),
                    ],
                    Space::with_width(Length::Fill),
                    iced::widget::toggler(form.living_background_enabled)
                        .on_toggle(Message::ConfigLivingBackgroundToggled)
                        .width(Length::Shrink)
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center),
                Space::with_height(Length::Fill),
            ]
            .spacing(8)
            .width(Length::Fill),
        )
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(Color {
                a: 0.08,
                ..pal.accent
            })),
            border: Border {
                radius: 12.0.into(),
                width: 1.0,
                color: Color {
                    a: 0.15,
                    ..pal.accent
                },
            },
            ..Default::default()
        });

        let status_text = form.status.clone().unwrap_or_default();
        let save_btn = button("Save Changes")
            .on_press(Message::SaveConfig)
            .padding([10, 20])
            .style(primary_button_style(pal));
        let status = text(status_text)
            .size(12)
            .style(move |_| iced::widget::text::Style {
                color: Some(pal.accent),
            });

        column![
            header,
            Space::with_height(Length::Fixed(12.0)),
            content,
            Space::with_height(Length::Fixed(12.0)),
            row![save_btn, Space::with_width(Length::Fixed(12.0)), status]
                .align_y(iced::Alignment::Center),
        ]
        .spacing(4)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    /// Renders the Model Selector page with loading state and model list.
    fn settings_model_selector_page(&self, pal: PaletteColors) -> Element<'_, Message> {
        let header = text("Select Model")
            .size(18)
            .style(move |_| iced::widget::text::Style {
                color: Some(pal.text),
            });

        let content_items: Element<'_, Message> = if self.models_loading {
            // Show loading spinner
            column![
                Space::with_height(Length::Fixed(40.0)),
                row![
                    text(Bootstrap::ArrowRepeat.to_string())
                        .font(iced_fonts::BOOTSTRAP_FONT)
                        .size(16)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        }),
                    Space::with_width(Length::Fixed(8.0)),
                    text("Fetching models...")
                        .size(16)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        }),
                ]
                .align_y(iced::Alignment::Center),
                Space::with_height(Length::Fill),
            ]
            .align_x(iced::Alignment::Center)
            .width(Length::Fill)
            .into()
        } else if self.model_list.is_empty() {
            // No models available
            column![
                Space::with_height(Length::Fixed(40.0)),
                text("No models available")
                    .size(14)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    }),
                Space::with_height(Length::Fill),
            ]
            .align_x(iced::Alignment::Center)
            .width(Length::Fill)
            .into()
        } else {
            // Show model list as buttons
            let mut model_col = column![].spacing(4).width(Length::Fill);
            for model in &self.model_list {
                let model_name = model.clone();
                let model_display = model.clone();
                let is_selected = model == &self.config_form.model;
                let model_btn = button(
                    row![
                        text(if is_selected {
                            Bootstrap::CheckLg.to_string()
                        } else {
                            " ".to_string()
                        })
                        .font(iced_fonts::BOOTSTRAP_FONT)
                        .size(14)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(if is_selected {
                                pal.accent
                            } else {
                                Color::TRANSPARENT
                            })
                        }),
                        Space::with_width(Length::Fixed(8.0)),
                        text(model_display)
                            .size(13)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(pal.text)
                            }),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::SelectModel(model_name))
                .padding([10, 14])
                .width(Length::Fill)
                .style(move |_theme, status| {
                    let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                    iced::widget::button::Style {
                        background: Some(Background::Color(if is_selected {
                            Color {
                                a: 0.2,
                                ..pal.accent
                            }
                        } else if is_hovered {
                            Color { a: 0.1, ..pal.text }
                        } else {
                            Color::TRANSPARENT
                        })),
                        border: Border {
                            radius: 8.0.into(),
                            width: if is_selected { 1.0 } else { 0.0 },
                            color: Color {
                                a: 0.3,
                                ..pal.accent
                            },
                        },
                        text_color: pal.text,
                        ..Default::default()
                    }
                });
                model_col = model_col.push(model_btn);
            }
            iced::widget::scrollable(model_col)
                .height(Length::Fill)
                .into()
        };

        let content = container(content_items)
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(Color {
                    a: 0.08,
                    ..pal.accent
                })),
                border: Border {
                    radius: 12.0.into(),
                    width: 1.0,
                    color: Color {
                        a: 0.15,
                        ..pal.accent
                    },
                },
                ..Default::default()
            });

        column![header, Space::with_height(Length::Fixed(12.0)), content,]
            .spacing(4)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

fn main() -> iced::Result {
    iced::application("Arula Desktop", App::update, App::view)
        .subscription(App::subscription)
        .theme(|_| app_theme())
        .font(iced_fonts::BOOTSTRAP_FONT_BYTES)
        .run_with(App::init)
}
