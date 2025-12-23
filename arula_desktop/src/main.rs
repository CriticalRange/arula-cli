
use arula_core::utils::config::Config;
// Test edit - verifying edit tool functionality
use arula_core::SessionConfig;
use arula_core::{ConversationManager, ConversationMetadata};
use arula_desktop::animation::Spring;
use arula_desktop::canvas::{
    LiquidMenuBackground, LivingBackground, LoadingSpinner, SpinnerState, SpinnerType,
};
use arula_desktop::styles::{
    ai_bubble_style, chat_input_style,
    input_style, primary_button_style,
    transparent_style, user_bubble_style,
};
use arula_desktop::{
    app_theme_with_mode, collect_provider_options, palette_from_mode, ConfigForm, Dispatcher,
    LiquidMenuState, LivingBackgroundState, MessageEntry, PaletteColors, Session, SettingsMenuState,
    SettingsPage, TiltCardState, ThemeMode, UiEvent, MESSAGE_MAX_WIDTH, PAGE_SLIDE_DISTANCE,
    SETTINGS_CARD_WIDTH, TICK_INTERVAL_MS, TILT_CARD_COUNT,
    // Project context
    detect_project, generate_auto_manifest, is_ai_enhanced, DetectedProject,
};
use iced_fonts::bootstrap;

use chrono::Utc;
use iced::alignment::{Horizontal, Vertical};
use iced::time::{self, Duration};
use iced::widget::canvas::Canvas;
use iced::widget::text_editor;
use iced::widget::{
    button, checkbox, column, container, markdown, pick_list, row, scrollable, stack, text, text_input, Space,
};
use iced::{Background, Border, Color, Element, Font, Length, Point, Subscription, Task};
use rfd::FileDialog;
use std::collections::HashMap;
use std::path::PathBuf;

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
    /// Loading spinner state for various UI elements
    spinner_state: SpinnerState,
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
    /// Current working directory for the session
    current_directory: PathBuf,
    /// Whether the directory popup is shown
    show_directory_popup: bool,
    /// Whether the custom directory input is shown
    show_directory_custom_input: bool,
    /// Draft value for the custom directory input
    directory_draft: String,
    /// Recently used directories (most recent first)
    recent_directories: Vec<PathBuf>,
    /// Conversation manager for saving/loading conversations
    conversation_manager: ConversationManager,
    /// List of saved conversations
    saved_conversations: Vec<ConversationMetadata>,
    /// Whether the conversations sidebar is shown
    show_conversations: bool,
    /// Animation state for conversations sidebar visibility (0.0 = hidden, 1.0 = visible) - instant close
    conversations_sidebar_animation: f32,
    /// Animation state for layout offset for top/input bars (0.0 = closed, 1.0 = open) - smooth both ways
    conversations_layout_offset: f32,
    /// Persistent clipboard for Wayland compatibility
    clipboard: Option<arboard::Clipboard>,
    /// Draft value for custom model input in model selector
    custom_model_draft: String,
    /// Current theme mode (Light, Dark, Black)
    theme_mode: ThemeMode,
    /// Detected project info for current directory (cached)
    detected_project: Option<DetectedProject>,
    /// Whether the current PROJECT.manifest was AI-enhanced
    manifest_is_ai_enhanced: bool,
    /// Conversation starter suggestions (max 3)
    conversation_starters: Vec<String>,
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
    /// Custom model input draft changed
    CustomModelDraftChanged(String),
    /// Add custom model from draft input
    AddCustomModel,
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
    LinkClicked(markdown::Uri),
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
    /// Clear the current chat session
    ClearChat,
    /// Toggle the directory popup visibility
    ToggleDirectoryPopup,
    /// Open native file picker to select a directory
    OpenDirectoryPicker,
    /// Handle the result from the directory picker
    DirectoryPickerResult(Option<PathBuf>),
    /// Toggle showing the manual directory input
    ShowDirectoryCustomInput,
    /// Track manual directory input changes
    DirectoryDraftChanged(String),
    /// Apply a manual directory change
    ChangeDirectory,
    /// Select a recent directory from the popup
    SelectRecentDirectory(PathBuf),
    /// Close the directory popup
    CloseDirectoryPopup,
    /// Toggle conversations sidebar
    ToggleConversations,
    /// Load a conversation by ID
    LoadConversation(uuid::Uuid),
    /// Delete a conversation by ID
    DeleteConversation(uuid::Uuid),
    /// Refresh the conversations list
    RefreshConversations,
    /// Close conversations sidebar
    CloseConversations,
    /// Initialize project with AI (enhance PROJECT.manifest)
    InitializeProjectWithAI,
    /// Change theme mode (Light, Dark, Black)
    ThemeModeChanged(String),
    /// Theme submenu selection (Dark/Black)
    ThemeSubmenuChanged(String),
    /// Click on a conversation starter to use it
    StarterClicked(String),
}

/// Input field ID for focus management
fn input_id() -> iced::widget::Id {
    iced::widget::Id::new("chat-input")
}

/// Build enhanced system prompt
/// Note: PROJECT.manifest context is handled by arula_core's build_system_prompt()
fn build_enhanced_system_prompt(base_prompt: &str) -> String {
    // The base prompt is sufficient - PROJECT.manifest is loaded by arula_core
    base_prompt.to_string()
}

impl App {
    /// Initializes the application. Shows error dialog if initialization fails.
    fn init() -> (Self, Task<Message>) {
        match Self::try_init() {
            Ok(app) => Self::init_with_starters(app),
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

        let theme_mode = config_form.theme_mode;

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
            spinner_state: SpinnerState {
                tick: 0.0,
                spinner_type: SpinnerType::Orbital,
                size: 20.0,
                color: Color::from_rgba(0.4, 0.4, 0.4, 1.0),
                accent_color: Color::from_rgba(0.6, 0.6, 0.6, 1.0),
            },
            markdown_cache: HashMap::new(),
            tool_args_cache: HashMap::new(),
            tool_animations: HashMap::new(),
            stream_error: None,
            error_expanded: false,
            bash_output_lines: HashMap::new(),
            current_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            show_directory_popup: false,
            show_directory_custom_input: false,
            directory_draft: String::new(),
            recent_directories: Vec::new(),
            conversation_manager: ConversationManager::new()?,
            saved_conversations: Vec::new(),
            show_conversations: false,
            conversations_sidebar_animation: 0.0,
            conversations_layout_offset: 0.0,
            clipboard: arboard::Clipboard::new().ok(),
            custom_model_draft: String::new(),
            theme_mode,
            detected_project: {
                // Detect project on startup
                let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
                let detected = detect_project(&cwd);
                
                // Create manifest if it doesn't exist
                if detected.is_some() {
                    let manifest_path = cwd.join("PROJECT.manifest");
                    if !manifest_path.exists() {
                        if let Some(ref project) = detected {
                            let content = generate_auto_manifest(project);
                            let _ = std::fs::write(&manifest_path, content);
                        }
                    }
                }
                detected
            },
            manifest_is_ai_enhanced: {
                let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
                is_ai_enhanced(&cwd.join("PROJECT.manifest"))
            },
            conversation_starters: Vec::new(),
        })
    }

    /// Post-initialization hook to start loading conversation starters
    fn init_with_starters(app: Self) -> (Self, Task<Message>) {
        // Trigger async fetch of conversation starters (don't show until received)
        app.dispatcher.generate_conversation_starters();
        (app, iced::widget::operation::focus(input_id()))
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
            spinner_state: SpinnerState {
                tick: 0.0,
                spinner_type: SpinnerType::Orbital,
                size: 20.0,
                color: Color::from_rgba(0.4, 0.4, 0.4, 1.0),
                accent_color: Color::from_rgba(0.6, 0.6, 0.6, 1.0),
            },
            markdown_cache: HashMap::new(),
            tool_args_cache: HashMap::new(),
            tool_animations: HashMap::new(),
            stream_error: None,
            error_expanded: false,
            bash_output_lines: HashMap::new(),
            current_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            show_directory_popup: false,
            show_directory_custom_input: false,
            directory_draft: String::new(),
            recent_directories: Vec::new(),
            conversation_manager: ConversationManager::new().unwrap_or_else(|_| {
                // If we can't create the conversation manager, just use a dummy one
                // This shouldn't happen in normal circumstances
                panic!("Failed to create conversation manager")
            }),
            saved_conversations: Vec::new(),
            show_conversations: false,
            conversations_sidebar_animation: 0.0,
            conversations_layout_offset: 0.0,
            clipboard: arboard::Clipboard::new().ok(),
            custom_model_draft: String::new(),
            theme_mode: ThemeMode::default(),
            detected_project: None,
            manifest_is_ai_enhanced: false,
            conversation_starters: Vec::new(),
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
                return iced::widget::operation::focus(input_id());
            }
            Message::Received(ev) => return self.handle_ui_event(ev),
            Message::NewTab => {
                self.sessions.push(Session::new());
                self.current = self.sessions.len() - 1;
                // Fetch conversation starters for the new session
                self.dispatcher.generate_conversation_starters();
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

                // Update spinner animation
                self.spinner_state.tick += 0.016; // ~60fps

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

                // Animate conversations sidebar visibility - smooth open, instant close
                let target_conversations = if self.show_conversations { 1.0 } else { 0.0 };
                
                if target_conversations < self.conversations_sidebar_animation {
                    // Closing: snap to 0 immediately (sidebar disappears)
                    self.conversations_sidebar_animation = target_conversations;
                } else {
                    // Opening: use smooth easing animation
                    self.conversations_sidebar_animation += (target_conversations - self.conversations_sidebar_animation) * 0.15;
                    if (self.conversations_sidebar_animation - target_conversations).abs() < 0.005 {
                        self.conversations_sidebar_animation = target_conversations;
                    }
                }
                
                // Animate layout offset for top/input bars - smooth both ways
                self.conversations_layout_offset += (target_conversations - self.conversations_layout_offset) * 0.15;
                if (self.conversations_layout_offset - target_conversations).abs() < 0.005 {
                    self.conversations_layout_offset = target_conversations;
                }

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
            Message::CustomModelDraftChanged(draft) => {
                self.custom_model_draft = draft;
            }
            Message::AddCustomModel => {
                let trimmed = self.custom_model_draft.trim();
                if !trimmed.is_empty() {
                    // Set the custom model as the selected model
                    self.config_form.model = trimmed.to_string();
                    // Clear the draft
                    self.custom_model_draft.clear();
                    // Navigate back to Provider page
                    self.settings_state.navigate_to(SettingsPage::Provider);
                }
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
            Message::ThemeModeChanged(mode) => {
                if let Some(theme_mode) = ThemeMode::from_name(&mode) {
                    println!("Theme mode changed to: {:?}", theme_mode);
                    self.theme_mode = theme_mode;
                    self.config_form.theme_mode = theme_mode;
                    return Task::none();
                }
            }
            Message::ThemeSubmenuChanged(submenu) => {
                // Handle Dark/Black submenu selection
                match submenu.as_str() {
                    "Dark" => {
                        println!("Theme submenu changed to: Dark");
                        self.theme_mode = ThemeMode::Dark;
                        self.config_form.theme_mode = ThemeMode::Dark;
                    }
                    "Black" => {
                        println!("Theme submenu changed to: Black");
                        self.theme_mode = ThemeMode::Black;
                        self.config_form.theme_mode = ThemeMode::Black;
                    }
                    _ => {}
                }
                return Task::none();
            }
            Message::StarterClicked(starter) => {
                // Set the draft to the starter and send it
                self.draft = starter;
                // Trigger send prompt
                return Task::done(Message::SendPrompt);
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
                        return iced::widget::operation::focus(input_id());
                    }
                }
            }
            Message::ToggleToolCollapse(key) => {
                // Get or create animation spring for this tool
                // Important: we need to know the DEFAULT state to create the spring correctly
                // Thinking bubbles (finalized) default to collapsed, tools default to expanded
                let is_thinking = key.contains(":")
                    && self.sessions.iter().enumerate().any(|(sidx, session)| {
                        session.messages.iter().enumerate().any(|(midx, msg)| {
                            let msg_key = format!("{}:{}", sidx, midx);
                            msg_key == key
                                && msg.is_thinking()
                                && msg.thinking_duration_secs.is_some()
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
                // Use persistent clipboard to prevent Wayland "dropped too quickly" issue
                if let Some(ref mut clipboard) = self.clipboard {
                    let _ = clipboard.set_text(text);
                }
            }
            Message::ClearChat => {
                let session_id = self.sessions.get(self.current).map(|s| s.id);
                let was_streaming = self
                    .sessions
                    .get(self.current)
                    .map(|s| s.is_streaming)
                    .unwrap_or(false);
                let tool_call_ids: Vec<String> = self
                    .sessions
                    .get(self.current)
                    .map(|s| {
                        s.messages
                            .iter()
                            .filter_map(|m| m.tool_call_id.clone())
                            .collect()
                    })
                    .unwrap_or_default();

                if was_streaming {
                    if let Some(id) = session_id {
                        self.dispatcher.stop_stream(id);
                    }
                }

                // Drop all cached UI state for this session so the next chat is pristine
                let prefix = format!("{}:", self.current);
                self.message_editors.retain(|k, _| !k.starts_with(&prefix));
                self.markdown_cache.retain(|k, _| !k.starts_with(&prefix));
                self.tool_animations.retain(|k, _| !k.starts_with(&prefix));

                if let Some(id) = session_id {
                    self.tool_args_cache.remove(&id);
                }
                for tool_call_id in tool_call_ids {
                    self.bash_output_lines.remove(&tool_call_id);
                }

                self.stream_error = None;
                self.error_expanded = false;
                self.draft.clear();

                if let Some(session) = self.sessions.get_mut(self.current) {
                    *session = Session::new();
                }
                // Fetch conversation starters for the fresh session
                self.dispatcher.generate_conversation_starters();

                return iced::widget::operation::focus(input_id());
            }
            Message::ToggleDirectoryPopup => {
                self.show_directory_popup = !self.show_directory_popup;
                if !self.show_directory_popup {
                    // Reset custom input state when closing
                    self.show_directory_custom_input = false;
                    self.directory_draft.clear();
                }
            }
            Message::CloseDirectoryPopup => {
                self.show_directory_popup = false;
                self.show_directory_custom_input = false;
                self.directory_draft.clear();
            }
            Message::ShowDirectoryCustomInput => {
                self.show_directory_custom_input = true;
                // Pre-fill with current directory
                self.directory_draft = self.current_directory.display().to_string();
            }
            Message::DirectoryDraftChanged(s) => {
                self.directory_draft = s;
            }
            Message::ChangeDirectory => {
                let path = PathBuf::from(&self.directory_draft);
                self.apply_directory_selection(path);
            }
            Message::OpenDirectoryPicker => {
                let start_dir = self.current_directory.clone();
                return Task::future(async move {
                    let path = FileDialog::new().set_directory(start_dir).pick_folder();
                    Message::DirectoryPickerResult(path)
                });
            }
            Message::DirectoryPickerResult(path) => {
                if let Some(path) = path {
                    self.apply_directory_selection(path);
                }
            }
            Message::SelectRecentDirectory(path) => {
                self.apply_directory_selection(path);
            }
            Message::ToggleConversations => {
                self.show_conversations = !self.show_conversations;
                if self.show_conversations {
                    return Task::future(async move {
                        Message::RefreshConversations
                    });
                }
            }
            Message::RefreshConversations => {
                if let Ok(conversations) = self.conversation_manager.list_conversations() {
                    self.saved_conversations = conversations;
                }
            }
            Message::LoadConversation(conversation_id) => {
                if let Ok(conversation) = self.conversation_manager.load_conversation(conversation_id) {
                    // Create a new session from the loaded events
                    let new_session = Session::from_events(conversation_id, &conversation.events);
                    
                    // Add the new session
                    self.sessions.push(new_session);
                    self.current = self.sessions.len() - 1;
                    
                    // Close the conversations sidebar
                    self.show_conversations = false;
                    
                    // Clear the draft
                    self.draft.clear();
                    
                    // Focus the input
                    return iced::widget::operation::focus(input_id());
                }
            }
            Message::DeleteConversation(conversation_id) => {
                if let Err(err) = self.conversation_manager.delete_conversation(conversation_id) {
                    eprintln!("Failed to delete conversation: {}", err);
                } else {
                    // Refresh the list
                    return Task::future(async move {
                        Message::RefreshConversations
                    });
                }
            }
            Message::CloseConversations => {
                self.show_conversations = false;
            }
            Message::InitializeProjectWithAI => {
                // Build a prompt for AI to enhance the manifest
                let project_name = self.detected_project
                    .as_ref()
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "this project".to_string());
                
                let prompt = format!(
                    "Please analyze {} and enhance the PROJECT.manifest file.\n\n\
                    First, read the existing PROJECT.manifest file to see what's already there.\n\
                    Then explore the key files to understand the project architecture.\n\n\
                    Update the manifest with:\n\
                    1. A comprehensive 'essence' section describing what the project does\n\
                    2. Key architecture patterns and design decisions\n\
                    3. Important gotchas, pitfalls, and conventions\n\
                    4. Common development tasks and how to approach them\n\n\
                    CRITICAL: The FIRST LINE of the file MUST be exactly:\n\
                    `# AI-ENHANCED by ARULA`\n\n\
                    Then add a comment with today's date, and include all the enhanced content.\n\
                    Keep the existing detected information but enrich it with your understanding.",
                    project_name
                );
                
                // Add as user message and trigger send
                if let Some(session) = self.sessions.get_mut(self.current) {
                    if !session.is_streaming {
                        session.add_user_message(prompt.clone(), Utc::now().to_rfc3339());
                        session.set_streaming(true);
                        
                        let session_config = SessionConfig {
                            system_prompt: build_enhanced_system_prompt(&self.config_form.system_prompt),
                            model: self.config.get_model(),
                            max_tokens: self.config_form.max_tokens as u32,
                            temperature: self.config_form.temperature,
                        };
                        
                        let history = session.get_chat_history();
                        let history_opt = if history.is_empty() { None } else { Some(history) };
                        
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
                }
                return iced::widget::operation::focus(input_id());
            }
        }
        Task::none()
    }

    fn apply_directory_selection(&mut self, path: PathBuf) {
        if !path.exists() || !path.is_dir() {
            return;
        }

        if std::env::set_current_dir(&path).is_ok() {
            self.recent_directories.retain(|p| p != &path);
            self.recent_directories.insert(0, path.clone());
            self.recent_directories.truncate(10);

            self.current_directory = path.clone();
            self.show_directory_popup = false;
            self.show_directory_custom_input = false;
            self.directory_draft.clear();
            
            // Auto-detect project and create manifest if needed
            self.detected_project = detect_project(&path);
            
            let manifest_path = path.join("PROJECT.manifest");
            if !manifest_path.exists() {
                // Create auto-generated manifest if we detected a project
                if let Some(ref project) = self.detected_project {
                    let content = generate_auto_manifest(project);
                    let _ = std::fs::write(&manifest_path, content);
                }
            }
            
            // Check if manifest is AI-enhanced
            self.manifest_is_ai_enhanced = is_ai_enhanced(&manifest_path);
        }
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
            UiEvent::ConversationStarters(starters) => {
                self.conversation_starters = starters;
                return Task::none();
            }
            UiEvent::ConversationTitle(title) => {
                // Update the current session's title
                if let Some(s) = self.sessions.get_mut(self.current) {
                    s.set_title(title);
                }
            }
            UiEvent::UserMessage { content: _, timestamp: _ } => {
                // Clear starters when conversation starts
                self.conversation_starters.clear();
                // User messages are handled locally in the session, no action needed
            }
            UiEvent::AiMessage { content: _, timestamp: _ } => {
                // AI messages are handled via Token events, no action needed
            }
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
                    
                    // Check if this is a non-streaming response (complete response at once)
                    let is_non_streaming = !session.is_streaming() && is_final;
                    
                    if is_non_streaming {
                        // For non-streaming responses, use add_ai_message to create separate bubbles
                        session.add_ai_message(delta, Utc::now().to_rfc3339());
                    } else {
                        // For streaming responses, use append_ai_message
                        session.append_ai_message(delta, Utc::now().to_rfc3339());
                    }

                    // Get or create the text editor content for the AI message
                    let msg_idx = session.messages.len() - 1;
                    let key = format!("{}:{}", idx, msg_idx);

                    // Handle message editor updates differently for streaming vs non-streaming
                    if is_non_streaming {
                        // For non-streaming responses, always update the editor content
                        self.message_editors.insert(
                            key.clone(),
                            text_editor::Content::with_text(&session.messages[msg_idx].content),
                        );
                    } else {
                        // For streaming responses, only update if it doesn't exist or if this is final
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
                    }

                    // Update markdown cache for AI messages
                    // Parse markdown on final token or periodically during streaming
                    let should_update_md = is_final || !self.markdown_cache.contains_key(&key);
                    if should_update_md && session.messages[msg_idx].is_ai() {
                        let content = &session.messages[msg_idx].content;
                        let items: Vec<markdown::Item> = markdown::parse(content).collect();
                        self.markdown_cache.insert(key, items);
                    }

                    // Handle final token differently for streaming vs non-streaming
                    if is_final {
                        if !is_non_streaming {
                            // For streaming responses, flush any remaining buffer content
                            session.flush_ai_buffer(Utc::now().to_rfc3339());
                        }
                        session.set_streaming(false);
                        // Re-focus input when response completes
                        return iced::widget::operation::focus(input_id());
                    }
                }
            }
            UiEvent::StreamFinished(id) => {
                if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                    // Flush any remaining AI content from the buffer
                    s.flush_ai_buffer(Utc::now().to_rfc3339());
                    s.set_streaming(false);
                    
                    // Save the conversation
                    let events = s.to_ui_events();
                    if let Err(err) = self.conversation_manager.save_conversation(
                        s.id,
                        &events,
                        self.config.get_model(),
                    ) {
                        eprintln!("Failed to save conversation: {}", err);
                    }
                }
                // Re-focus input when stream finishes
                return iced::widget::operation::focus(input_id());
            }
            UiEvent::StreamErrored(id, err) => {
                eprintln!("stream error {id}: {err}");
                // Store error for display to user
                self.stream_error = Some(err);
                if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                    s.set_streaming(false);
                }
                // Re-focus input on error
                return iced::widget::operation::focus(input_id());
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
        let stream = self.dispatcher.subscription().map(Message::Received);
        let ticks = time::every(Duration::from_millis(TICK_INTERVAL_MS)).map(|_| Message::Tick);
        Subscription::batch(vec![stream, ticks])
    }

    fn view(&self) -> Element<'_, Message> {
        let pal = palette_from_mode(self.theme_mode);
        
        // Debug: print current theme mode
        static LAST_THEME: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(255);
        let current_theme_id = match self.theme_mode {
            ThemeMode::Light => 0,
            ThemeMode::Dark => 1,
            ThemeMode::Black => 2,
        };
        if LAST_THEME.load(std::sync::atomic::Ordering::Relaxed) != current_theme_id {
            println!("🎨 View rendering with theme: {:?}, background: {:?}", self.theme_mode, pal.background);
            LAST_THEME.store(current_theme_id, std::sync::atomic::Ordering::Relaxed);
        }

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

        // Calculate width offset for top bar and input bar based on layout offset (smooth animation)
        let sidebar_width = 340.0 * self.conversations_layout_offset;

        // Build main layer with top bar, chat content, optional typing indicator, and input
        let mut main_content: Vec<Element<'_, Message>> = vec![self.top_bar(pal, sidebar_width)];
        main_content.push(self.chat_panel(pal));

        // Add typing indicator above input when streaming
        if is_streaming {
            main_content.push(self.typing_indicator(pal));
        }

        main_content.push(self.input_area(pal, sidebar_width));

        let main_layer = column(main_content)
            .width(Length::Fill)
            .height(Length::Fill);

        let progress = self.menu_state.progress();
        let overlay = if progress > 0.01 {
            self.settings_overlay(pal).into()
        } else {
            Space::new().into()
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
            } else if error.chars().count() > 60 && !self.error_expanded {
                // Find safe character boundary
                let truncate_at = error
                    .char_indices()
                    .take(60)
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(0);
                format!("{}...", &error[..truncate_at])
            } else {
                error.clone()
            };

            let chevron = if self.error_expanded {
                bootstrap::chevron_down()
            } else {
                bootstrap::chevron_right()
            };

            // Header row (always visible, clickable to expand)
            let header_row = row![
                text("!")
                    .size(18)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(Color {
                            r: 1.0,
                            g: 0.8,
                            b: 0.2,
                            a: 1.0
                        })
                    }),
                Space::new().width(Length::Fixed(8.0)),
                chevron
                    .size(12)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(Color { a: 0.7, ..pal.text })
                    }),
                Space::new().width(Length::Fixed(8.0)),
                text(if self.error_expanded {
                    "Error Details".to_string()
                } else {
                    short_error
                })
                .size(13)
                .style(move |_| iced::widget::text::Style {
                    color: Some(pal.text)
                }),
                Space::new().width(Length::Fill),
                button(
                    text("×")
                        .size(14)
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
                    scrollable(text(error.as_str()).size(12).font(Font::MONOSPACE).style(
                        move |_| iced::widget::text::Style {
                            color: Some(Color {
                                r: 1.0,
                                g: 0.7,
                                b: 0.7,
                                a: 1.0,
                            }),
                        },
                    ))
                    .height(Length::Fixed(120.0)),
                )
                .padding([6, 16])
                .width(Length::Fill)
                .into()
            } else {
                Space::new().into()
            };

            let error_container = container(column![header_button, expanded_content].spacing(0))
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
                Space::new().height(Length::Fill),
                error_container,
                Space::new().height(Length::Fixed(80.0)), // Space above input
            ])
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .into()
        } else {
            Space::new().into()
        };

        let directory_popup = self.directory_popup(pal);
        let conversations_sidebar = self.conversations_sidebar(pal);

        // Add backdrop overlay for conversations sidebar
        let conversations_backdrop: Element<'_, Message> = if self.show_conversations {
            button(Space::new().width(Length::Fill).height(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_theme, _status| button::Style {
                    background: Some(Background::Color(Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: self.conversations_sidebar_animation * 0.3, // Darken backdrop when sidebar is open
                    })),
                    ..Default::default()
                })
                .on_press(Message::CloseConversations) // Close when clicking backdrop
                .into()
        } else {
            Space::new().into()
        };

        let content = stack(vec![
            background.into(),
            main_layer.into(),
            overlay,
            conversations_backdrop, // Add backdrop behind conversations sidebar
            directory_popup,
            conversations_sidebar,
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

    fn top_bar(&self, pal: PaletteColors, sidebar_width: f32) -> Element<'_, Message> {
        // ─────────────────────────────────────────────────────────────────
        // LEFT SIDE: Navigation buttons (icon-based for clean look)
        // ─────────────────────────────────────────────────────────────────

        // Conversations button (history icon)
        let conversations_active = self.show_conversations;
        let conversations_button = button(
            container(
                bootstrap::clock_history()
                    .size(18)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(if conversations_active { pal.accent } else { pal.muted })
                    })
            )
            .width(Length::Fixed(36.0))
            .height(Length::Fixed(36.0))
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
        )
        .on_press(Message::ToggleConversations)
        .padding(0)
        .style(move |_theme, status| {
            let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: Some(Background::Color(Color {
                    a: if conversations_active { 0.25 } else if is_hovered { 0.15 } else { 0.0 },
                    ..pal.accent
                })),
                border: Border {
                    radius: 10.0.into(),
                    ..Default::default()
                },
                text_color: pal.muted,
                ..Default::default()
            }
        });

        // New Chat button (plus icon)
        let new_chat_button = button(
            container(
                bootstrap::plus_lg()
                    .size(18)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    })
            )
            .width(Length::Fixed(36.0))
            .height(Length::Fixed(36.0))
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
        )
        .on_press(Message::ClearChat)
        .padding(0)
        .style(move |_theme, status| {
            let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: Some(Background::Color(Color {
                    a: if is_hovered { 0.15 } else { 0.0 },
                    ..pal.accent
                })),
                border: Border {
                    radius: 10.0.into(),
                    ..Default::default()
                },
                text_color: pal.muted,
                ..Default::default()
            }
        });

        let left_buttons = row![
            conversations_button,
            Space::new().width(Length::Fixed(4.0)),
            new_chat_button,
        ]
        .align_y(iced::Alignment::Center);

        // ─────────────────────────────────────────────────────────────────
        // CENTER: Directory selector (pill-shaped with folder icon)
        // ─────────────────────────────────────────────────────────────────

        let dir_name = self.current_directory
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_else(|| self.current_directory.to_str().unwrap_or("/"));
        
        let is_popup_open = self.show_directory_popup;
        let directory_button = button(
            row![
                bootstrap::folder()
                    .size(14)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(if is_popup_open { pal.accent } else { pal.muted })
                    }),
                Space::new().width(Length::Fixed(8.0)),
                text(dir_name)
                    .size(13)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.text)
                    }),
                Space::new().width(Length::Fixed(6.0)),
                bootstrap::chevron_down()
                    .size(10)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(Color { a: 0.5, ..pal.muted })
                    }),
            ]
            .align_y(iced::Alignment::Center),
        )
        .on_press(Message::ToggleDirectoryPopup)
        .padding([8, 14])
        .style(move |_theme, status| {
            let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
            button::Style {
                background: Some(Background::Color(Color {
                    a: if is_popup_open { 0.2 } else if is_hovered { 0.1 } else { 0.0 },
                    ..pal.accent
                })),
                border: Border {
                    radius: 12.0.into(),
                    width: 1.0,
                    color: Color {
                        a: if is_popup_open { 0.4 } else if is_hovered { 0.25 } else { 0.15 },
                        ..pal.border
                    },
                },
                text_color: pal.text,
                ..Default::default()
            }
        });

        // ─────────────────────────────────────────────────────────────────
        // RIGHT SIDE: Optional AI Initialize button
        // ─────────────────────────────────────────────────────────────────

        let show_init_button = self.detected_project.is_some() && !self.manifest_is_ai_enhanced;
        let init_ai_button: Option<Element<'_, Message>> = if show_init_button {
            Some(
                button(
                    row![
                        bootstrap::stars()
                            .size(14)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(pal.accent)
                            }),
                        Space::new().width(Length::Fixed(6.0)),
                        text("AI Init")
                            .size(12)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(pal.text)
                            }),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::InitializeProjectWithAI)
                .padding([8, 14])
                .style(move |_theme, status| {
                    let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                    button::Style {
                        background: Some(Background::Color(Color {
                            a: if is_hovered { 0.3 } else { 0.2 },
                            ..pal.accent
                        })),
                        border: Border {
                            radius: 12.0.into(),
                            width: 1.0,
                            color: Color {
                                a: if is_hovered { 0.7 } else { 0.5 },
                                ..pal.accent
                            },
                        },
                        text_color: pal.text,
                        ..Default::default()
                    }
                })
                .into()
            )
        } else {
            None
        };

        // ─────────────────────────────────────────────────────────────────
        // TOP BAR LAYOUT: Glassmorphism container
        // ─────────────────────────────────────────────────────────────────

        let mut top_row = row![
            left_buttons,
            Space::new().width(Length::Fixed(12.0)),
            directory_button,
        ]
        .align_y(iced::Alignment::Center);
        
        // Push spacer and optional AI button to right
        top_row = top_row.push(Space::new().width(Length::Fill));
        
        if let Some(ai_btn) = init_ai_button {
            top_row = top_row.push(ai_btn);
        }

        let top_bar_content = container(top_row)
            .padding([6, 10])
            .width(Length::Shrink);

        let top_bar = container(top_bar_content)
            .style(move |_| container::Style {
                background: Some(Background::Color(Color {
                    a: 0.5,
                    ..pal.surface_raised
                })),
                border: Border {
                    color: Color {
                        a: 0.3,
                        ..pal.border
                    },
                    width: 1.0,
                    radius: 16.0.into(),
                },
                ..Default::default()
            });

        // Outer container with padding - adjust left padding based on sidebar width
        let left_pad = if sidebar_width > 1.0 { sidebar_width } else { 0.0 };
        container(top_bar)
            .padding(iced::padding::Padding {
                top: 12.0,
                right: 16.0,
                bottom: 12.0,
                left: left_pad.max(0.0) + 16.0,
            })
            .width(Length::Fill)
            .height(Length::Shrink)
            .style(move |_| container::Style {
                background: None,
                ..Default::default()
            })
            .into()
    }

    /// Creates the directory popup overlay - improved UX
    fn directory_popup(&self, pal: PaletteColors) -> Element<'_, Message> {
        if !self.show_directory_popup {
            return Space::new().into();
        }

        let mut popup_content: Vec<Element<'_, Message>> = Vec::new();

        // ─────────────────────────────────────────────────────────────────
        // HEADER
        // ─────────────────────────────────────────────────────────────────
        
        popup_content.push(
            row![
                text("Select Directory")
                    .size(14)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.text)
                    }),
                Space::new().width(Length::Fill),
                button(
                    bootstrap::x_lg()
                        .size(14)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        })
                )
                .on_press(Message::ToggleDirectoryPopup)
                .padding(4)
                .style(move |_theme, status| {
                    let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                    button::Style {
                        background: Some(Background::Color(Color {
                            a: if is_hovered { 0.2 } else { 0.0 },
                            ..pal.muted
                        })),
                        border: Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
            ]
            .align_y(iced::Alignment::Center)
            .into()
        );

        popup_content.push(Space::new().height(Length::Fixed(12.0)).into());

        // ─────────────────────────────────────────────────────────────────
        // PRIMARY ACTION: Browse button (large and prominent)
        // ─────────────────────────────────────────────────────────────────

        let browse_button = button(
            row![
                bootstrap::folder_plus()
                    .size(20)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.text)
                    }),
                Space::new().width(Length::Fixed(12.0)),
                column![
                    text("Browse...")
                        .size(14)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.text)
                        }),
                    text("Open native file picker")
                        .size(11)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        }),
                ]
                .spacing(2),
            ]
            .align_y(iced::Alignment::Center),
        )
        .on_press(Message::OpenDirectoryPicker)
        .padding([14, 16])
        .width(Length::Fill)
        .style(move |_theme, status| {
            let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
            button::Style {
                background: Some(Background::Color(Color {
                    a: if is_hovered { 0.35 } else { 0.25 },
                    ..pal.accent
                })),
                border: Border {
                    radius: 10.0.into(),
                    width: 1.0,
                    color: Color {
                        a: if is_hovered { 0.6 } else { 0.4 },
                        ..pal.accent
                    },
                },
                text_color: pal.text,
                ..Default::default()
            }
        });

        popup_content.push(browse_button.into());
        popup_content.push(Space::new().height(Length::Fixed(16.0)).into());

        // ─────────────────────────────────────────────────────────────────
        // QUICK ACCESS: Home, Documents, Desktop
        // ─────────────────────────────────────────────────────────────────

        popup_content.push(
            text("Quick Access")
                .size(11)
                .style(move |_| iced::widget::text::Style {
                    color: Some(pal.muted)
                })
                .into()
        );
        popup_content.push(Space::new().height(Length::Fixed(6.0)).into());

        // Quick access buttons in a row
        let home_dir = dirs::home_dir().unwrap_or_default();
        let home_clone = home_dir.clone();
        let docs_dir = dirs::document_dir().unwrap_or_else(|| home_dir.join("Documents"));
        let docs_clone = docs_dir.clone();
        let desktop_dir = dirs::desktop_dir().unwrap_or_else(|| home_dir.join("Desktop"));
        let desktop_clone = desktop_dir.clone();

        let quick_access_row = row![
            // Home
            button(
                column![
                    bootstrap::house()
                        .size(18)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        }),
                    text("Home")
                        .size(11)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        }),
                ]
                .spacing(4)
                .align_x(iced::Alignment::Center)
            )
            .on_press(Message::SelectRecentDirectory(home_clone))
            .padding([10, 16])
            .style(move |_theme, status| {
                let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                button::Style {
                    background: Some(Background::Color(Color {
                        a: if is_hovered { 0.2 } else { 0.1 },
                        ..pal.surface_raised
                    })),
                    border: Border {
                        radius: 8.0.into(),
                        width: 1.0,
                        color: Color {
                            a: if is_hovered { 0.3 } else { 0.15 },
                            ..pal.border
                        },
                    },
                    ..Default::default()
                }
            }),
            Space::new().width(Length::Fixed(8.0)),
            // Documents
            button(
                column![
                    bootstrap::file_earmark_text()
                        .size(18)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        }),
                    text("Docs")
                        .size(11)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        }),
                ]
                .spacing(4)
                .align_x(iced::Alignment::Center)
            )
            .on_press(Message::SelectRecentDirectory(docs_clone))
            .padding([10, 16])
            .style(move |_theme, status| {
                let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                button::Style {
                    background: Some(Background::Color(Color {
                        a: if is_hovered { 0.2 } else { 0.1 },
                        ..pal.surface_raised
                    })),
                    border: Border {
                        radius: 8.0.into(),
                        width: 1.0,
                        color: Color {
                            a: if is_hovered { 0.3 } else { 0.15 },
                            ..pal.border
                        },
                    },
                    ..Default::default()
                }
            }),
            Space::new().width(Length::Fixed(8.0)),
            // Desktop
            button(
                column![
                    bootstrap::display()
                        .size(18)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        }),
                    text("Desktop")
                        .size(11)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        }),
                ]
                .spacing(4)
                .align_x(iced::Alignment::Center)
            )
            .on_press(Message::SelectRecentDirectory(desktop_clone))
            .padding([10, 16])
            .style(move |_theme, status| {
                let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                button::Style {
                    background: Some(Background::Color(Color {
                        a: if is_hovered { 0.2 } else { 0.1 },
                        ..pal.surface_raised
                    })),
                    border: Border {
                        radius: 8.0.into(),
                        width: 1.0,
                        color: Color {
                            a: if is_hovered { 0.3 } else { 0.15 },
                            ..pal.border
                        },
                    },
                    ..Default::default()
                }
            }),
        ]
        .align_y(iced::Alignment::Center);

        popup_content.push(quick_access_row.into());

        // ─────────────────────────────────────────────────────────────────
        // RECENT DIRECTORIES (if any)
        // ─────────────────────────────────────────────────────────────────

        if !self.recent_directories.is_empty() {
            popup_content.push(Space::new().height(Length::Fixed(16.0)).into());
            popup_content.push(
                text("Recent")
                    .size(11)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    })
                    .into()
            );
            popup_content.push(Space::new().height(Length::Fixed(6.0)).into());

            for dir in self.recent_directories.iter().take(4) {
                let dir_clone = dir.clone();
                let dir_name = dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_else(|| dir.to_str().unwrap_or("?"));
                
                // Truncate path for display
                let dir_path = dir.display().to_string();
                let display_path = if dir_path.chars().count() > 35 {
                    let truncate_at = dir_path
                        .char_indices()
                        .take(32)
                        .last()
                        .map(|(i, c)| i + c.len_utf8())
                        .unwrap_or(0);
                    format!("{}...", &dir_path[..truncate_at])
                } else {
                    dir_path
                };
                
                let recent_item = button(
                    row![
                        bootstrap::folder()
                            .size(14)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(pal.accent)
                            }),
                        Space::new().width(Length::Fixed(10.0)),
                        column![
                            text(dir_name)
                                .size(13)
                                .style(move |_| iced::widget::text::Style {
                                    color: Some(pal.text)
                                }),
                            text(display_path)
                                .size(10)
                                .style(move |_| iced::widget::text::Style {
                                    color: Some(Color { a: 0.6, ..pal.muted })
                                }),
                        ]
                        .spacing(2),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::SelectRecentDirectory(dir_clone))
                .padding([10, 12])
                .width(Length::Fill)
                .style(move |_theme, status| {
                    let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                    button::Style {
                        background: Some(Background::Color(Color {
                            a: if is_hovered { 0.15 } else { 0.05 },
                            ..pal.surface_raised
                        })),
                        border: Border {
                            radius: 8.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        text_color: pal.text,
                        ..Default::default()
                    }
                });
                
                popup_content.push(recent_item.into());
            }
        }

        // ─────────────────────────────────────────────────────────────────
        // MANUAL PATH INPUT (expandable)
        // ─────────────────────────────────────────────────────────────────

        popup_content.push(Space::new().height(Length::Fixed(12.0)).into());
        
        if self.show_directory_custom_input {
            let dir_input = text_input("Enter path (e.g. /home/user/project)", &self.directory_draft)
                .on_input(Message::DirectoryDraftChanged)
                .on_submit(Message::ChangeDirectory)
                .padding([10, 12])
                .size(13)
                .width(Length::Fill)
                .style(move |_theme, status| {
                    let is_focused = matches!(status, iced::widget::text_input::Status::Focused { .. });
                    iced::widget::text_input::Style {
                        background: Background::Color(Color {
                            a: 0.15,
                            ..pal.surface_raised
                        }),
                        border: Border {
                            radius: 8.0.into(),
                            width: 1.0,
                            color: if is_focused { pal.accent } else { Color { a: 0.3, ..pal.border } },
                        },
                        icon: pal.muted,
                        placeholder: pal.muted,
                        value: pal.text,
                        selection: Color { a: 0.3, ..pal.accent },
                    }
                });

            popup_content.push(dir_input.into());
            popup_content.push(Space::new().height(Length::Fixed(8.0)).into());
            
            let confirm_btn = button(
                text("Open")
                    .size(13)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.text)
                    })
            )
            .on_press(Message::ChangeDirectory)
            .padding([8, 20])
            .style(move |_theme, status| {
                let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                button::Style {
                    background: Some(Background::Color(Color {
                        a: if is_hovered { 0.4 } else { 0.3 },
                        ..pal.success
                    })),
                    border: Border {
                        radius: 8.0.into(),
                        ..Default::default()
                    },
                    text_color: pal.text,
                    ..Default::default()
                }
            });

            popup_content.push(
                row![
                    Space::new().width(Length::Fill),
                    confirm_btn,
                ]
                .into()
            );
        } else {
            // Toggle to show manual input
            let manual_button = button(
                row![
                    bootstrap::keyboard()
                        .size(12)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        }),
                    Space::new().width(Length::Fixed(6.0)),
                    text("Type path manually")
                        .size(11)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        }),
                ]
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::ShowDirectoryCustomInput)
            .padding([6, 10])
            .style(move |_theme, status| {
                let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                button::Style {
                    background: Some(Background::Color(Color {
                        a: if is_hovered { 0.1 } else { 0.0 },
                        ..pal.surface_raised
                    })),
                    border: Border {
                        radius: 6.0.into(),
                        ..Default::default()
                    },
                    text_color: pal.muted,
                    ..Default::default()
                }
            });

            popup_content.push(manual_button.into());
        }

        // ─────────────────────────────────────────────────────────────────
        // POPUP CONTAINER
        // ─────────────────────────────────────────────────────────────────

        let popup = container(
            column(popup_content)
                .spacing(2)
                .padding(16)
        )
        .width(Length::Fixed(340.0))
        .style(move |_| container::Style {
            background: Some(Background::Color(Color {
                a: 0.95,
                ..pal.background
            })),
            border: Border {
                radius: 16.0.into(),
                width: 1.0,
                color: Color { a: 0.4, ..pal.border },
            },
            ..Default::default()
        });

        // Position the popup below the top bar
        container(
            column![
                Space::new().height(Length::Fixed(70.0)), // Below top bar
                row![
                    Space::new().width(Length::Fixed(100.0)),
                    popup,
                ],
            ]
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    /// Creates the conversations sidebar - modern relaxing design
    /// Animations: Staggered Cascade (opacity), Content Parallax (timing), Glow Reveal
    /// Uses SLIDE ANIMATION - sidebar stays full width, slides from off-screen (no squishing!)
    fn conversations_sidebar(&self, pal: PaletteColors) -> Element<'_, Message> {
        // ─────────────────────────────────────────────────────────────────
        // ANIMATION CALCULATIONS
        // ─────────────────────────────────────────────────────────────────
        
        let t = self.conversations_sidebar_animation;
        
        // Cubic ease-out for smooth deceleration
        let eased = 1.0 - (1.0 - t).powi(3);
        
        // Fixed width - never changes! Content stays at proper size
        let sidebar_width = 340.0;
        
        // Slide offset: animate from -340px (off-screen left) to 0px (visible)
        // When t=0: offset = -340 (hidden)
        // When t=1: offset = 0 (fully visible)
        let slide_offset = -sidebar_width * (1.0 - eased);
        
        // Content Parallax: Header appears faster (different timing curve)
        let header_progress = (t * 1.3).min(1.0); // 30% faster timing
        let header_opacity = 1.0 - (1.0 - header_progress).powi(2); // Faster fade-in

        let mut sidebar_content: Vec<Element<'_, Message>> = Vec::new();

        // ─────────────────────────────────────────────────────────────────
        // HEADER: Parallax effect (slides in faster)
        // ─────────────────────────────────────────────────────────────────
        
        let header = container(
            row![
                // Title
                row![
                    bootstrap::chat_left_text()
                        .size(18)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.accent)
                        }),
                    Space::new().width(Length::Fixed(10.0)),
                    text("Conversations")
                        .size(16)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.text)
                        }),
                ]
                .align_y(iced::Alignment::Center),
                
                Space::new().width(Length::Fill),
                
                // Refresh button (icon only)
                button(
                    bootstrap::arrow_clockwise()
                        .size(16)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        })
                )
                .on_press(Message::RefreshConversations)
                .padding(8)
                .style(move |_theme, status| {
                    let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                    button::Style {
                        background: Some(Background::Color(Color {
                            a: if is_hovered { 0.15 } else { 0.0 },
                            ..pal.accent
                        })),
                        border: Border {
                            radius: 8.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
                
                Space::new().width(Length::Fixed(4.0)),
                
                // Close button (icon only)
                button(
                    bootstrap::x_lg()
                        .size(16)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        })
                )
                .on_press(Message::CloseConversations)
                .padding(8)
                .style(move |_theme, status| {
                    let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                    button::Style {
                        background: Some(Background::Color(Color {
                            a: if is_hovered { 0.2 } else { 0.0 },
                            ..pal.surface_raised
                        })),
                        border: Border {
                            radius: 8.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
            ]
            .align_y(iced::Alignment::Center)
        )
        .padding([16, 16])
        .width(Length::Fill);

        // Apply parallax timing to header (fades in faster than items)
        let header_with_parallax = container(header)
            .width(Length::Fill)
            .style(move |_| container::Style {
                text_color: Some(Color {
                    a: header_opacity,
                    ..pal.text
                }),
                ..Default::default()
            });

        sidebar_content.push(header_with_parallax.into());
        sidebar_content.push(Space::new().height(Length::Fixed(8.0)).into());

        // ─────────────────────────────────────────────────────────────────
        // CONVERSATION LIST
        // ─────────────────────────────────────────────────────────────────

        if self.saved_conversations.is_empty() {
            // Empty state - calm and inviting
            let empty_state = container(
                column![
                    bootstrap::chat_dots()
                        .size(48)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(Color { a: 0.3, ..pal.accent })
                        }),
                    Space::new().height(Length::Fixed(16.0)),
                    text("No conversations yet")
                        .size(15)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.text)
                        }),
                    Space::new().height(Length::Fixed(4.0)),
                    text("Start chatting and your\nconversations will appear here")
                        .size(12)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        })
                        .align_x(iced::Alignment::Center),
                ]
                .spacing(0)
                .align_x(iced::Alignment::Center)
            )
            .padding([60, 20])
            .width(Length::Fill)
            .align_x(Horizontal::Center);
            
            sidebar_content.push(empty_state.into());
        } else {
            // Conversation cards with Staggered Cascade animation
            let cards = column(
                    self.saved_conversations
                        .iter()
                        .enumerate()
                        .map(|(index, conversation)| {
                            let conv_id = conversation.id;
                            
                            // Truncate title safely
                            let title = if conversation.title.chars().count() > 30 {
                                let truncate_at = conversation.title
                                    .char_indices()
                                    .take(27)
                                    .last()
                                    .map(|(i, c)| i + c.len_utf8())
                                    .unwrap_or(0);
                                format!("{}...", conversation.title[..truncate_at].trim())
                            } else {
                                conversation.title.clone()
                            };

                            // Staggered Cascade: Each item fades in with increasing delay
                            let item_delay = 0.1 + (index as f32 * 0.06); // Base delay + stagger
                            let item_progress = ((t - item_delay) / (1.0 - item_delay)).clamp(0.0, 1.0);
                            let item_opacity = 1.0 - (1.0 - item_progress).powi(2); // Ease out

                            let card = button(
                                row![
                                    // Icon
                                    container(
                                        bootstrap::chat()
                                            .size(16)
                                            .style(move |_| iced::widget::text::Style {
                                                color: Some(pal.accent)
                                            })
                                    )
                                    .width(Length::Fixed(32.0))
                                    .height(Length::Fixed(32.0))
                                    .align_x(Horizontal::Center)
                                    .align_y(Vertical::Center)
                                    .style(move |_| container::Style {
                                        background: Some(Background::Color(Color {
                                            a: 0.15,
                                            ..pal.accent
                                        })),
                                        border: Border {
                                            radius: 8.0.into(),
                                            ..Default::default()
                                        },
                                        ..Default::default()
                                    }),
                                    
                                    Space::new().width(Length::Fixed(12.0)),
                                    
                                    // Content
                                    column![
                                        text(title)
                                            .size(13)
                                            .style(move |_| iced::widget::text::Style {
                                                color: Some(pal.text)
                                            }),
                                        row![
                                            text(format!("{} msgs", conversation.message_count))
                                                .size(11)
                                                .style(move |_| iced::widget::text::Style {
                                                    color: Some(Color { a: 0.6, ..pal.muted })
                                                }),
                                            Space::new().width(Length::Fixed(8.0)),
                                            text("•")
                                                .size(11)
                                                .style(move |_| iced::widget::text::Style {
                                                    color: Some(Color { a: 0.4, ..pal.muted })
                                                }),
                                            Space::new().width(Length::Fixed(8.0)),
                                            text(conversation.relative_time())
                                                .size(11)
                                                .style(move |_| iced::widget::text::Style {
                                                    color: Some(Color { a: 0.6, ..pal.muted })
                                                }),
                                        ]
                                        .align_y(iced::Alignment::Center),
                                    ]
                                    .spacing(4),
                                ]
                                .align_y(iced::Alignment::Center)
                            )
                            .on_press(Message::LoadConversation(conv_id))
                            .padding([12, 14])
                            .width(Length::Fill)
                            .style(move |_theme, status| {
                                let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                                button::Style {
                                    background: Some(Background::Color(Color {
                                        a: if is_hovered { 0.12 } else { 0.05 },
                                        ..pal.surface_raised
                                    })),
                                    border: Border {
                                        radius: 12.0.into(),
                                        width: 1.0,
                                        color: Color {
                                            a: if is_hovered { 0.25 } else { 0.1 },
                                            ..pal.border
                                        },
                                    },
                                    text_color: pal.text,
                                    ..Default::default()
                                }
                            });
                            // Wrap card with staggered opacity
                            container(card)
                                .width(Length::Fill)
                                .style(move |_| container::Style {
                                    text_color: Some(Color {
                                        a: item_opacity,
                                        ..pal.text
                                    }),
                                    ..Default::default()
                                })
                                .into()
                        })
                        .collect::<Vec<Element<'_, Message>>>()
                )
                .spacing(6)
                .width(Length::Fixed(sidebar_width - 24.0));  // Account for padding (12*2)
            
            let conversations_container = container(cards)
                .padding([0, 12])
                .width(Length::Fill);  // This can fill since parent is fixed

            sidebar_content.push(conversations_container.into());
        }

        // ─────────────────────────────────────────────────────────────────
        // SIDEBAR CONTAINER: Glassmorphism + Glow Reveal
        // ─────────────────────────────────────────────────────────────────

        // Scrollable content with FIXED width - never changes size!
        let scroll_content = scrollable(column(sidebar_content))
            .width(Length::Fixed(sidebar_width))
            .height(Length::Fill);

        // Inner container - ALWAYS full 340px width, never squishes
        let sidebar_inner = container(scroll_content)
            .width(Length::Fixed(sidebar_width))
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(Color {
                    a: 0.95 * eased,
                    ..pal.background
                })),
                border: Border {
                    radius: 16.0.into(),
                    width: 1.0,
                    color: Color {
                        a: 0.15 * eased,
                        ..pal.border
                    },
                },
                shadow: iced::Shadow {
                    color: Color {
                        r: pal.accent.r,
                        g: pal.accent.g,
                        b: pal.accent.b,
                        a: 0.3 * eased,
                    },
                    offset: iced::Vector { x: 8.0, y: 0.0 },
                    blur_radius: 24.0,
                },
                ..Default::default()
            });

        // Outer container with SLIDE animation - moves sidebar from off-screen
        // Only skip rendering when sidebar visibility animation is fully at 0 (completely closed)
        // Otherwise render (it will be clipped if off-screen) to sync with sidebar visibility
        if self.conversations_sidebar_animation <= 0.001 {
            return Space::new().into();
        }
        
        // Create off-screen slide effect
        // When slide_offset is negative, sidebar is off-screen to the left
        let x_position = slide_offset;
        
        // Use row with spacer to push sidebar, wrap in clipping container
        container(
            row![
                Space::new().width(Length::Fixed(x_position.max(0.0))),
                container(sidebar_inner)
                    .width(Length::Fixed(sidebar_width))
                    .height(Length::Fill),
            ]
            .width(Length::Fill)
            .height(Length::Fill)
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)  // Clip anything outside the viewport
        .into()
    }

    fn chat_panel(&self, pal: PaletteColors) -> Element<'_, Message> {
        let session = &self.sessions[self.current];

        if session.messages.is_empty() && !session.is_streaming {
            // Build starter boxes if available
            let starters: Vec<Element<'_, Message>> = self.conversation_starters
                .iter()
                .map(|starter| {
                    button(
                        text(starter)
                            .size(14)
                            .width(Length::Fill)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(pal.text),
                            })
                    )
                    .padding(12)
                    .style(move |_theme, _status| {
                        let is_hovered = _status == iced::widget::button::Status::Hovered;
                        iced::widget::button::Style {
                            background: Some(Background::Color(Color {
                                r: pal.surface.r,
                                g: pal.surface.g,
                                b: pal.surface.b,
                                a: if is_hovered { 0.7 } else { 0.5 },
                            })),
                            border: Border {
                                color: pal.border,
                                width: 1.0,
                                radius: 8.0.into(),
                            },
                            shadow: iced::Shadow {
                                color: Color::BLACK,
                                offset: iced::Vector::new(0.0, 2.0),
                                blur_radius: 4.0,
                            },
                            text_color: pal.text,
                            ..Default::default()
                        }
                    })
                    .width(Length::Fill)
                    .on_press(Message::StarterClicked(starter.clone()))
                    .into()
                })
                .collect();

            let starters_row = if starters.is_empty() {
                row![].spacing(8)
            } else {
                row(starters)
                    .spacing(12)
                    .width(Length::Fill)
            };

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
                    // Add spacing and starters
                    container(starters_row)
                        .padding(40)
                        .width(Length::Fill),
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
        // Create a loading spinner with orbital animation
        let spinner = Canvas::new(LoadingSpinner::new(SpinnerState {
            tick: self.spinner_state.tick,
            spinner_type: SpinnerType::Orbital,
            size: 12.0,
            color: pal.accent,
            accent_color: Color {
                r: pal.accent.r * 0.7,
                g: pal.accent.g * 0.7,
                b: pal.accent.b * 0.7,
                a: pal.accent.a,
            },
        }))
        .width(Length::Fixed(24.0))
        .height(Length::Fixed(24.0));

        // Wrap in a bubble-like container
        let indicator_content = container(
            row![
                spinner,
                Space::new().width(Length::Fixed(8.0)),
                text("aruling...")
                    .size(14)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    }),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([12, 18])
        .style(move |_| container::Style {
            background: Some(Background::Color(Color {
                a: 0.08,
                ..pal.surface
            })),
            border: Border {
                radius: 16.0.into(),
                width: 1.0,
                color: Color {
                    a: 0.15,
                    ..pal.surface
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
                    markdown::Settings::with_style(markdown::Style::from_palette(iced::Theme::TokyoNightStorm.palette())),
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
            bootstrap::clipboard()
                .size(12)
                .style(move |_| iced::widget::text::Style {
                    color: Some(Color {
                        a: fade_opacity * 0.6,
                        ..pal.muted
                    }),
                }),
        )
        .on_press(Message::CopyToClipboard(content_to_copy))
        .padding([2, 4])
        .style(move |_theme, status| {
            let hover_opacity = if matches!(status, button::Status::Hovered) {
                1.0
            } else {
                0.6
            };
            button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                border: Border::default(),
                text_color: Color {
                    a: fade_opacity * hover_opacity,
                    ..pal.muted
                },
                ..Default::default()
            }
        });

        // Bottom row with timestamp and copy button
        let bottom_row = row![timestamp, Space::new().width(Length::Fill), copy_button]
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
            row![Space::new().width(Length::Fill), styled_bubble].into()
        } else if is_tool {
            // Terminal-style tool bubble with collapsible content
            return self.terminal_style_tool_bubble(msg_idx, message, &key, pal, fade_opacity);
        } else if is_thinking {
            // Thinking bubble - collapsible when finalized with "Thought for X seconds"
            return self.thinking_style_bubble(msg_idx, message, &key, pal, fade_opacity);
        } else {
            let base_style_fn = ai_bubble_style(pal, false); // Pass false since we handle opacity manually here
            let styled_bubble = bubble.style(move |t| dynamic_style(base_style_fn(t)));
            row![styled_bubble, Space::new().width(Length::Fill)].into()
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
        let tool_type = if content.starts_with("○ Shell")
            || content.starts_with("◆ Shell")
            || content.contains("execute_bash")
        {
            ToolType::Shell
        } else if content.starts_with("□ Edit")
            || content.starts_with("◆ Edit")
            || content.contains("edit_file")
        {
            ToolType::EditFile
        } else if content.starts_with("□ Write")
            || content.starts_with("◆ Write")
            || content.contains("write_file")
        {
            ToolType::WriteFile
        } else if content.starts_with("○ Read")
            || content.starts_with("◆ Read")
            || content.contains("read_file")
        {
            ToolType::ReadFile
        } else if content.starts_with("◇ List")
            || content.starts_with("◆ List")
            || content.contains("list_directory")
        {
            ToolType::ListDirectory
        } else if content.starts_with("○ Search")
            || content.starts_with("◆ Search")
            || (content.contains("Search") && !content.contains("Web"))
        {
            ToolType::Search
        } else if content.starts_with("⭕ Web")
            || content.starts_with("◆ Web")
            || content.contains("web_search")
        {
            ToolType::WebSearch
        } else if content.starts_with("◊ MCP")
            || content.starts_with("◆ MCP")
            || content.contains("mcp_call")
        {
            ToolType::Mcp
        } else if content.starts_with("○ Vision")
            || content.starts_with("◆ Vision")
            || content.contains("visioneer")
        {
            ToolType::Vision
        } else {
            ToolType::Other
        };

        let is_shell = tool_type == ToolType::Shell;
        let is_edit = tool_type == ToolType::EditFile;
        let is_read = tool_type == ToolType::ReadFile;
        let is_search = tool_type == ToolType::Search || tool_type == ToolType::WebSearch;
        let _is_list = tool_type == ToolType::ListDirectory;

        // Check if operation completed (has ✓ or ✗)
        let has_checkmark = content.contains('✓');
        let has_error = content.contains('✗');

        // Tool-specific theming: (accent_color, header_bg, content_bg, icon, label)
        // Using muted, neutral colors with subtle tints for a calmer UI
        let neutral_header_bg = Color {
            r: 0.10,
            g: 0.10,
            b: 0.11,
            a: fade_opacity * 0.95,
        };
        let neutral_terminal_bg = Color {
            r: 0.06,
            g: 0.06,
            b: 0.07,
            a: fade_opacity * 0.95,
        };

        let (bubble_accent_color, header_bg_color, terminal_bg_color, tool_icon, header_label) =
            match tool_type {
                ToolType::Shell => (
                    Color {
                        r: 0.55,
                        g: 0.55,
                        b: 0.65,
                        a: fade_opacity * 0.85,
                    }, // Muted grayish-purple
                    neutral_header_bg,
                    neutral_terminal_bg,
                    bootstrap::terminal(),
                    "",
                ),
                ToolType::ReadFile => (
                    Color {
                        r: 0.5,
                        g: 0.55,
                        b: 0.65,
                        a: fade_opacity * 0.85,
                    }, // Muted grayish-blue
                    neutral_header_bg,
                    neutral_terminal_bg,
                    bootstrap::file_earmark_text(),
                    "Read",
                ),
                ToolType::WriteFile => (
                    Color {
                        r: 0.5,
                        g: 0.6,
                        b: 0.55,
                        a: fade_opacity * 0.85,
                    }, // Muted grayish-green
                    neutral_header_bg,
                    neutral_terminal_bg,
                    bootstrap::file_earmark_plus(),
                    "Wrote",
                ),
                ToolType::EditFile => (
                    Color {
                        r: 0.65,
                        g: 0.58,
                        b: 0.5,
                        a: fade_opacity * 0.85,
                    }, // Muted grayish-orange
                    neutral_header_bg,
                    neutral_terminal_bg,
                    bootstrap::file_earmark_diff(),
                    "Edited",
                ),
                ToolType::ListDirectory => (
                    Color {
                        r: 0.5,
                        g: 0.58,
                        b: 0.58,
                        a: fade_opacity * 0.85,
                    }, // Muted grayish-teal
                    neutral_header_bg,
                    neutral_terminal_bg,
                    bootstrap::folder_fill(),
                    "Listed",
                ),
                ToolType::Search => (
                    Color {
                        r: 0.52,
                        g: 0.58,
                        b: 0.62,
                        a: fade_opacity * 0.85,
                    }, // Muted grayish-cyan
                    neutral_header_bg,
                    neutral_terminal_bg,
                    bootstrap::search(),
                    "Searched",
                ),
                ToolType::WebSearch => (
                    Color {
                        r: 0.5,
                        g: 0.55,
                        b: 0.62,
                        a: fade_opacity * 0.85,
                    }, // Muted grayish-blue
                    neutral_header_bg,
                    neutral_terminal_bg,
                    bootstrap::globe(),
                    "Searched Web",
                ),
                ToolType::Mcp => (
                    Color {
                        r: 0.58,
                        g: 0.52,
                        b: 0.6,
                        a: fade_opacity * 0.85,
                    }, // Muted grayish-purple
                    neutral_header_bg,
                    neutral_terminal_bg,
                    bootstrap::plug_fill(),
                    "Ran MCP",
                ),
                ToolType::Vision => (
                    Color {
                        r: 0.6,
                        g: 0.55,
                        b: 0.58,
                        a: fade_opacity * 0.85,
                    }, // Muted grayish-pink
                    neutral_header_bg,
                    neutral_terminal_bg,
                    bootstrap::eye_fill(),
                    "Viewed",
                ),
                ToolType::Other => (
                    Color {
                        r: 0.55,
                        g: 0.55,
                        b: 0.55,
                        a: fade_opacity * 0.85,
                    }, // Neutral gray
                    neutral_header_bg,
                    neutral_terminal_bg,
                    bootstrap::gear_fill(),
                    "Ran",
                ),
            };

        // Parse search-specific data (path and query)
        let (search_path, search_query) = if tool_type == ToolType::Search {
            // Try to extract path and query from content
            let path_query: (Option<String>, Option<String>) = (None, None);
            
            // Look for path: pattern
            let path = content
                .find("path:")
                .and_then(|idx| {
                    let after_path = &content[idx + 5..];
                    after_path
                        .trim()
                        .trim_start_matches('"')
                        .split('"')
                        .next()
                        .map(|s| s.trim().to_string())
                });
            
            // Look for query: pattern
            let query = content
                .find("query:")
                .and_then(|idx| {
                    let after_query = &content[idx + 6..];
                    after_query
                        .trim()
                        .trim_start_matches('"')
                        .split('"')
                        .next()
                        .map(|s| s.trim().to_string())
                });
            
            (path.or(path_query.0), query.or(path_query.1))
        } else {
            (None, None)
        };

        // Extract filename from Edit operations
        let edit_filename = if tool_type == ToolType::EditFile {
            // Try to extract path from content
            content
                .find("path:")
                .and_then(|idx| {
                    let after = &content[idx + 5..];
                    let start = after.find('"').map(|s| s + 1)?;
                    let end = after[start..].find('"')?;
                    let full_path = &after[start..start + end];
                    // Get just the filename
                    full_path.split('/').last().map(|s| s.to_string())
                })
        } else {
            None
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
            } else if before_status.chars().count() > 50 {
                // Find a safe character boundary for truncation
                let truncate_at = before_status
                    .char_indices()
                    .take(47)
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(0);
                format!("{}...", &before_status[..truncate_at])
            } else {
                before_status.to_string()
            }
        };

        // For Edit files, override command_display to be empty so we don't show raw content
        let command_display = if tool_type == ToolType::EditFile { 
            String::new()
        } else {
            command_display
        };

        // Extract result text (everything after ✓ or ✗)
        let result_text = if has_checkmark {
            content.split('✓').nth(1).map(|s| s.trim().to_string())
        } else if has_error {
            content.split('✗').nth(1).map(|s| s.trim().to_string())
        } else {
            None
        };

        // Extract result count for search tools
        let result_count = if (tool_type == ToolType::Search || tool_type == ToolType::WebSearch) 
            && result_text.is_some() {
            result_text.as_ref().and_then(|text| {
                // Try to extract count from patterns like "Found X files:" or "X results:"
                let text_lower = text.to_lowercase();
                
                // Look for "Found X files:" pattern
                if let Some(found_idx) = text_lower.find("found ") {
                    let after_found = &text_lower[found_idx + 6..];
                    if let Some(space_idx) = after_found.find(' ') {
                        let count_str = &after_found[..space_idx];
                        if let Ok(count) = count_str.parse::<usize>() {
                            return Some(count);
                        }
                    }
                }
                
                // Look for just a number followed by "files" or "results"
                for word in text.split_whitespace() {
                    if let Ok(count) = word.parse::<usize>() {
                        return Some(count);
                    }
                }
                
                // Count the number of file entries shown
                let file_count = text.lines()
                    .filter(|line| line.contains("📄") || line.contains(".rs") || line.contains(".md"))
                    .count();
                
                if file_count > 0 {
                    Some(file_count)
                } else {
                    None
                }
            })
        } else {
            None
        };

        // Parse edit file diff data - extract from result JSON
        let (lines_added, lines_removed, diff_content) = if tool_type == ToolType::EditFile {
            // Try to parse result as JSON to get line counts and diff
            if let Some(result) = result_text.as_ref() {
                if result.trim().starts_with('{') {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(result) {
                        let added = json.get("lines_added")
                            .or_else(|| json.get("inserted"))
                            .and_then(|v| v.as_u64())
                            .map(|v| v as usize);
                        
                        let removed = json.get("lines_removed")
                            .or_else(|| json.get("deleted"))
                            .and_then(|v| v.as_u64())
                            .map(|v| v as usize);
                        
                        // Get diff content if available
                        let diff = json.get("diff")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        
                        (added, removed, diff)
                    } else {
                        (None, None, None)
                    }
                } else {
                    (None, None, None)
                }
            } else {
                (None, None, None)
            }
        } else {
            (None, None, None)
        };

        // Status icon using Bootstrap icons
        let status_icon = if has_checkmark {
            bootstrap::check_lg()
        } else if has_error {
            bootstrap::x_lg()
        } else {
            bootstrap::circle()
        };

        // Muted status colors - less vibrant green/red
        let status_color = if has_checkmark {
            Color {
                r: 0.5,
                g: 0.7,
                b: 0.55,
                a: fade_opacity * 0.9,
            }
        } else if has_error {
            Color {
                r: 0.75,
                g: 0.5,
                b: 0.5,
                a: fade_opacity * 0.9,
            }
        } else {
            bubble_accent_color
        };

        // Dropdown chevron using Bootstrap
        let chevron_icon = if is_collapsed {
            bootstrap::chevron_right()
        } else {
            bootstrap::chevron_down()
        };

        // For read file, include the result summary in the header display
        let header_detail = if is_read {
            // Show full info: "filename • X chars"
            if let Some(ref result) = result_text {
                format!("{} • {}", command_display, result)
            } else {
                command_display.clone()
            }
        } else if tool_type == ToolType::EditFile {
            // Show "Edited {filename} +{added} -{removed}"
            let filename = edit_filename.as_deref().unwrap_or("file");
            if let (Some(added), Some(removed)) = (lines_added, lines_removed) {
                format!("Edited {} +{} -{}", filename, added, removed)
            } else {
                format!("Edited {}", filename)
            }
        } else if tool_type == ToolType::Search || tool_type == ToolType::WebSearch {
            // Show "Searched in [path] for [query]" or "Searched Web for [query]"
            if tool_type == ToolType::WebSearch {
                if let Some(query) = &search_query {
                    format!("Searched Web for \"{}\"", if query.chars().count() > 40 {
                        format!("{}...", query.chars().take(37).collect::<String>())
                    } else {
                        query.clone()
                    })
                } else {
                    command_display.clone()
                }
            } else if let (Some(path), Some(query)) = (&search_path, &search_query) {
                format!("Searched in \"{}\" for \"{}\"", 
                    if path.chars().count() > 25 {
                        format!("{}...", path.chars().take(22).collect::<String>())
                    } else {
                        path.clone()
                    },
                    if query.chars().count() > 30 {
                        format!("{}...", query.chars().take(27).collect::<String>())
                    } else {
                        query.clone()
                    }
                )
            } else if let Some(path) = &search_path {
                format!("Searched in \"{}\"", path)
            } else {
                command_display.clone()
            }
        } else {
            command_display.clone()
        };

        // Right-side status text (for search tools: result count, for edit: +X/-Y)
        let right_status_text = if tool_type == ToolType::EditFile {
            if let (Some(added), Some(removed)) = (lines_added, lines_removed) {
                Some(format!("+{} / -{}", added, removed))
            } else {
                None
            }
        } else if (tool_type == ToolType::Search || tool_type == ToolType::WebSearch) 
            && result_count.is_some() {
            Some(format!("{} Results", result_count.unwrap()))
        } else {
            None
        };

        // Build header row - chevron goes on the RIGHT side (after status icon)
        // ReadFile doesn't get a chevron since it's not expandable
        // Shell commands don't show a label, just the icon and command
        let mut header_row = row![
            tool_icon
                .size(14)
                .style(move |_| iced::widget::text::Style {
                    color: Some(bubble_accent_color)
                }),
        ]
        .align_y(iced::Alignment::Center);

        // Only add label if not empty (shell commands have empty label)
        if !header_label.is_empty() {
            header_row = header_row.push(Space::new().width(Length::Fixed(6.0)));
            header_row = header_row.push(
                text(header_label)
                    .size(13)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(bubble_accent_color)
                    }),
            );
        }

        header_row = header_row.push(Space::new().width(Length::Fixed(8.0)));
        header_row = header_row.push(
            text(header_detail)
                .size(12)
                .font(Font::MONOSPACE)
                .style(move |_| iced::widget::text::Style {
                    color: Some(Color {
                        a: fade_opacity * 0.7,
                        ..pal.text
                    })
                }),
        );
        header_row = header_row.push(Space::new().width(Length::Fill));
        
        // Add result count for search tools or +/- for edits (before status icon)
        if let Some(ref count_text) = right_status_text {
            // For edit files, color the + and - differently
            if tool_type == ToolType::EditFile {
                // Split the text to color +X green and -Y red
                if let Some(plus_idx) = count_text.find('+') {
                    if let Some(space_idx) = count_text[plus_idx..].find(' ') {
                        let added_part = &count_text[plus_idx..plus_idx + space_idx];
                        let removed_part = &count_text[plus_idx + space_idx..];
                        
                        // Added lines in green
                        header_row = header_row.push(
                            text(added_part.to_string())
                                .size(11)
                                .style(move |_| iced::widget::text::Style {
                                    color: Some(Color {
                                        r: 0.5,
                                        g: 0.7,
                                        b: 0.55,
                                        a: fade_opacity * 0.8,
                                    }),
                                }),
                        );
                        
                        // Removed lines in red
                        header_row = header_row.push(
                            text(removed_part.to_string())
                                .size(11)
                                .style(move |_| iced::widget::text::Style {
                                    color: Some(Color {
                                        r: 0.75,
                                        g: 0.5,
                                        b: 0.5,
                                        a: fade_opacity * 0.8,
                                    }),
                                }),
                        );
                    } else {
                        header_row = header_row.push(
                            text(count_text.clone())
                                .size(11)
                                .style(move |_| iced::widget::text::Style {
                                    color: Some(Color {
                                        a: fade_opacity * 0.6,
                                        ..pal.text
                                    }),
                                }),
                        );
                    }
                } else {
                    header_row = header_row.push(
                        text(count_text.clone())
                            .size(11)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(Color {
                                    a: fade_opacity * 0.6,
                                    ..pal.text
                                }),
                            }),
                    );
                }
            } else {
                // Default styling for other tools
                header_row = header_row.push(
                    text(count_text.clone())
                        .size(11)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(Color {
                                a: fade_opacity * 0.6,
                                ..pal.text
                            }),
                        }),
                );
            }
            header_row = header_row.push(Space::new().width(Length::Fixed(6.0)));
        }
        
        header_row = header_row.push(
            status_icon
                .size(14)
                .style(move |_| iced::widget::text::Style {
                    color: Some(status_color)
                }),
        );
        header_row = header_row.width(Length::Fill);

        // Add chevron on right side for expandable tools (not read file)
        if !is_read {
            header_row = header_row.push(Space::new().width(Length::Fixed(8.0)));
            header_row = header_row.push(
                chevron_icon
                    .size(12)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(Color {
                            a: fade_opacity * 0.7,
                            ..pal.text
                        }),
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
                        radius: if is_collapsed {
                            8.0.into()
                        } else {
                            iced::border::Radius::new(0.0).top(8.0)
                        },
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
            Space::new().into()
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

            // Result color - muted tones
            let result_glow_color = if has_checkmark {
                Color {
                    r: 0.55,
                    g: 0.72,
                    b: 0.58,
                    a: content_opacity * 0.9,
                } // Muted green
            } else if has_error {
                Color {
                    r: 0.75,
                    g: 0.52,
                    b: 0.52,
                    a: content_opacity * 0.9,
                } // Muted red
            } else {
                Color {
                    r: 0.6,
                    g: 0.6,
                    b: 0.6,
                    a: content_opacity * 0.85,
                } // Neutral gray
            };

            // Command line with prompt and command (skip for search tools - already in header)
            let mut terminal_column: iced::widget::Column<'_, Message> = if is_search {
                // For search tools, skip the command row since it's shown in the header
                column![].spacing(4)
            } else {
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
                column![command_row].spacing(4)
            };

            // Check for streaming bash output lines first (keyed by tool_call_id)
            let streaming_lines: Option<&Vec<(String, bool)>> = message
                .tool_call_id
                .as_ref()
                .and_then(|id| self.bash_output_lines.get(id));

            if let Some(lines) = streaming_lines {
                // Use streaming lines - show each with proper color
                // stdout = muted green, stderr = muted orange
                for (line, is_stderr) in lines.iter() {
                    let line_color = if *is_stderr {
                        Color {
                            r: 0.72,
                            g: 0.55,
                            b: 0.48,
                            a: content_opacity * 0.9,
                        } // Muted orange for stderr
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
                        // Use the pre-calculated diff if available, otherwise show result
                        let content_to_show = diff_content.as_ref().unwrap_or(result);
                        
                        for line in content_to_show.lines() {
                            let line_color = if line.starts_with('+') || line.starts_with("+ ") {
                                Color {
                                    r: 0.4,
                                    g: 1.0,
                                    b: 0.5,
                                    a: content_opacity,
                                } // Green for additions
                            } else if line.starts_with('-') || line.starts_with("- ") {
                                Color {
                                    r: 1.0,
                                    g: 0.4,
                                    b: 0.4,
                                    a: content_opacity,
                                } // Red for deletions
                            } else if line.starts_with("@@") || line.contains("line") {
                                Color {
                                    r: 0.6,
                                    g: 0.8,
                                    b: 1.0,
                                    a: content_opacity,
                                } // Blue for line markers
                            } else {
                                Color {
                                    r: 0.7,
                                    g: 0.7,
                                    b: 0.7,
                                    a: content_opacity,
                                } // Gray for context
                            };

                            terminal_column = terminal_column.push(
                                text(line.to_string())
                                    .size(12)
                                    .font(Font::MONOSPACE)
                                    .style(
                                        move |_| iced::widget::text::Style {
                                            color: Some(line_color),
                                        },
                                    ),
                            );
                        }
                    } else if is_search {
                        // For search tools with diff, show the result
                        for line in result.lines() {
                            // For search tools, replace "Found X files:" with "X Results"
                            let display_line = line
                                .replace("Found ", "")
                                .replace(" files:", " Results")
                                .replace(" file:", " Result");

                            terminal_column = terminal_column.push(
                                text(display_line)
                                    .size(12)
                                    .font(Font::MONOSPACE)
                                    .style(move |_| iced::widget::text::Style {
                                        color: Some(result_glow_color),
                                    }),
                            );
                        }
                    } else {
                        // Default rendering for other tools
                        for line in result.lines() {
                            terminal_column = terminal_column.push(
                                text(line.to_string())
                                    .size(12)
                                    .font(Font::MONOSPACE)
                                    .style(move |_| iced::widget::text::Style {
                                        color: Some(result_glow_color),
                                    }),
                            );
                        }
                    }
                }
            } else if is_shell && !has_checkmark && !has_error {
                // Shell command running but no output yet - show a running indicator
                let running_color = Color {
                    r: 0.6,
                    g: 0.6,
                    b: 0.65,
                    a: content_opacity * 0.8,
                };
                terminal_column = terminal_column.push(
                    text("Running...")
                        .size(12)
                        .font(Font::MONOSPACE)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(running_color),
                        }),
                );
            }

            // Wrap in container with tool-themed background
            // Animate background opacity
            let animated_bg = Color {
                a: terminal_bg_color.a * expand_progress,
                ..terminal_bg_color
            };
            let terminal_inner = container(terminal_column)
                .padding([10, 14])
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(animated_bg)),
                    border: Border {
                        radius: iced::border::Radius::new(0.0).bottom(8.0),
                        width: 1.0,
                        color: Color {
                            a: bubble_accent_color.a * 0.3 * expand_progress,
                            ..bubble_accent_color
                        },
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

        row![bubble, Space::new().width(Length::Fill)].into()
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
        let expand_progress =
            spring
                .map(|s| s.position)
                .unwrap_or(if is_finalized { 0.0 } else { 1.0 });
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

        // Build header row
        let mut header_row = row![
            bootstrap::lightbulb()
                .size(14)
                .style(move |_| iced::widget::text::Style {
                    color: Some(accent_color)
                }),
            Space::new().width(Length::Fixed(8.0)),
        ]
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

        // Add text elements with different colors
        if let Some(_duration) = message.thinking_duration_secs {
            header_row = header_row.push(
                text("Thought")
                    .size(13)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(accent_color)
                    })
            );

            if is_collapsed {
                // Show preview of content (max ~50 chars)
                let preview = if message.content.len() > 50 {
                    format!("{}...", message.content.chars().take(47).collect::<String>())
                } else {
                    message.content.clone()
                };
                header_row = header_row.push(
                    text(" ") // Space between "Thought" and preview
                        .size(13)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(accent_color)
                        })
                );
                header_row = header_row.push(
                    text(preview)
                        .size(13)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(Color {
                                r: 0.6,
                                g: 0.6,
                                b: 0.6,
                                a: 1.0,
                            })
                        })
                );
            }
        } else {
            header_row = header_row.push(
                text("Thinking...")
                    .size(13)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(accent_color)
                    })
            );
        }

        header_row = header_row.push(Space::new().width(Length::Fill));

        // Add chevron for finalized thinking (expandable)
        if is_finalized {
            let chevron = if is_collapsed {
                bootstrap::chevron_right()
            } else {
                bootstrap::chevron_down()
            };
            header_row = header_row.push(
                chevron
                    .size(12)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(Color {
                            a: fade_opacity * 0.7,
                            ..pal.text
                        }),
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
                        radius: if is_collapsed {
                            8.0.into()
                        } else {
                            iced::border::Radius::new(0.0).top(8.0)
                        },
                        width: 1.0,
                        color: Color {
                            a: accent_color.a * 0.5,
                            ..accent_color
                        },
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
                        color: Color {
                            a: accent_color.a * 0.5,
                            ..accent_color
                        },
                    },
                    text_color: pal.text,
                    ..Default::default()
                })
        };

        // Content area (only for expanded finalized thinking)
        // Uses expand_progress for smooth animation
        let thinking_content: Element<'_, Message> = if !is_finalized || expand_progress < 0.01 {
            Space::new().into()
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
            let animated_bg = Color {
                a: content_bg.a * expand_progress,
                ..content_bg
            };
            let content_inner = container(text(&message.content).size(13).style(move |_| {
                iced::widget::text::Style {
                    color: Some(text_color),
                }
            }))
            .padding([10, 14])
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(animated_bg)),
                border: Border {
                    radius: iced::border::Radius::new(0.0).bottom(8.0),
                    width: 1.0,
                    color: Color {
                        a: accent_color.a * 0.3 * expand_progress,
                        ..accent_color
                    },
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
        let bubble =
            container(column![header, thinking_content].spacing(0)).max_width(MESSAGE_MAX_WIDTH);

        row![bubble, Space::new().width(Length::Fill)].into()
    }

    fn input_area(&self, pal: PaletteColors, sidebar_width: f32) -> Element<'_, Message> {
        // Check if current session is streaming
        let is_streaming = self
            .sessions
            .get(self.current)
            .map(|s| s.is_streaming)
            .unwrap_or(false);

        // Check if menu is open
        let menu_open = self.menu_state.is_open();

        // ─────────────────────────────────────────────────────────────────
        // LEFT SIDE BUTTONS: Attachment tools
        // ─────────────────────────────────────────────────────────────────
        
        // Plus/Attachment button
        let attach_button = button(
            container(
                bootstrap::plus_lg()
                    .size(18)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    })
            )
            .width(Length::Fixed(36.0))
            .height(Length::Fixed(36.0))
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
        )
        .padding(0)
        .style(move |_theme, status| {
            let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: Some(Background::Color(Color {
                    a: if is_hovered { 0.2 } else { 0.0 },
                    ..pal.accent
                })),
                border: Border {
                    radius: 8.0.into(),
                    ..Default::default()
                },
                text_color: pal.muted,
                ..Default::default()
            }
        });
        // .on_press(Message::OpenAttachmentPicker)  // TODO: Implement later

        // Image/Photo button
        let image_button = button(
            container(
                bootstrap::image()
                    .size(16)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    })
            )
            .width(Length::Fixed(36.0))
            .height(Length::Fixed(36.0))
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
        )
        .padding(0)
        .style(move |_theme, status| {
            let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: Some(Background::Color(Color {
                    a: if is_hovered { 0.2 } else { 0.0 },
                    ..pal.accent
                })),
                border: Border {
                    radius: 8.0.into(),
                    ..Default::default()
                },
                text_color: pal.muted,
                ..Default::default()
            }
        });
        // .on_press(Message::OpenImagePicker)  // TODO: Implement later

        // Microphone button
        let mic_button = button(
            container(
                bootstrap::mic()
                    .size(16)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    })
            )
            .width(Length::Fixed(36.0))
            .height(Length::Fixed(36.0))
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
        )
        .padding(0)
        .style(move |_theme, status| {
            let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: Some(Background::Color(Color {
                    a: if is_hovered { 0.2 } else { 0.0 },
                    ..pal.accent
                })),
                border: Border {
                    radius: 8.0.into(),
                    ..Default::default()
                },
                text_color: pal.muted,
                ..Default::default()
            }
        });
        // .on_press(Message::ToggleMicrophone)  // TODO: Implement later

        let left_buttons = row![
            attach_button,
            image_button,
            mic_button,
        ]
        .spacing(2)
        .align_y(iced::Alignment::Center);

        // ─────────────────────────────────────────────────────────────────
        // CENTER: Text input field
        // ─────────────────────────────────────────────────────────────────
        
        let input_field = text_input("Message ARULA...", &self.draft)
            .id(input_id())
            .on_input(Message::DraftChanged)
            .on_submit(Message::SendPrompt)
            .padding([12, 8])
            .style(chat_input_style(pal))
            .width(Length::Fill);

        // ─────────────────────────────────────────────────────────────────
        // RIGHT SIDE: Settings gear + Send/Stop button
        // ─────────────────────────────────────────────────────────────────

        // Settings gear button (always shows gear, only opens settings - close button is at top)
        let settings_button = button(
            container(
                bootstrap::gear()
                    .size(16)
                    .style(move |_| iced::widget::text::Style {
                        // Dimmed when menu is open (use close button at top instead)
                        color: Some(if menu_open { Color { a: 0.3, ..pal.muted } } else { pal.muted })
                    })
            )
            .width(Length::Fixed(36.0))
            .height(Length::Fixed(36.0))
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
        )
        .on_press_maybe(if menu_open { None } else { Some(Message::ToggleSettings) })
        .padding(0)
        .style(move |_theme, status| {
            let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: Some(Background::Color(Color {
                    // No highlight when menu is open
                    a: if menu_open { 0.0 } else if is_hovered { 0.2 } else { 0.0 },
                    ..pal.accent
                })),
                border: Border {
                    radius: 8.0.into(),
                    ..Default::default()
                },
                text_color: pal.muted,
                ..Default::default()
            }
        });

        // Send/Stop button with modern pill shape
        let action_button: Element<'_, Message> = if is_streaming {
            // Stop button - red with square icon
            button(
                container(
                    bootstrap::stop_fill()
                        .size(18)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.text)
                        })
                )
                .width(Length::Fixed(44.0))
                .height(Length::Fixed(36.0))
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center)
            )
            .on_press(Message::StopStream)
            .padding(0)
            .style(move |_theme, status| {
                let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                let is_pressed = matches!(status, iced::widget::button::Status::Pressed);
                iced::widget::button::Style {
                    background: Some(Background::Color(Color {
                        a: if is_pressed { 0.7 } else if is_hovered { 0.9 } else { 0.8 },
                        ..pal.danger
                    })),
                    border: Border {
                        radius: 10.0.into(),
                        ..Default::default()
                    },
                    text_color: pal.text,
                    ..Default::default()
                }
            })
            .into()
        } else {
            // Send button - accent colored with arrow icon
            let has_content = !self.draft.trim().is_empty();
            button(
                container(
                    bootstrap::arrow_up()
                        .size(18)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(if has_content { pal.background } else { pal.muted })
                        })
                )
                .width(Length::Fixed(44.0))
                .height(Length::Fixed(36.0))
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center)
            )
            .on_press(Message::SendPrompt)
            .padding(0)
            .style(move |_theme, status| {
                let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                let is_pressed = matches!(status, iced::widget::button::Status::Pressed);
                
                if has_content {
                    iced::widget::button::Style {
                        background: Some(Background::Color(Color {
                            a: if is_pressed { 0.8 } else if is_hovered { 1.0 } else { 0.9 },
                            ..pal.accent
                        })),
                        border: Border {
                            radius: 10.0.into(),
                            ..Default::default()
                        },
                        text_color: pal.background,
                        ..Default::default()
                    }
                } else {
                    iced::widget::button::Style {
                        background: Some(Background::Color(Color {
                            a: 0.15,
                            ..pal.surface_raised
                        })),
                        border: Border {
                            radius: 10.0.into(),
                            ..Default::default()
                        },
                        text_color: pal.muted,
                        ..Default::default()
                    }
                }
            })
            .into()
        };

        let right_buttons = row![
            settings_button,
            Space::new().width(Length::Fixed(4.0)),
            action_button,
        ]
        .align_y(iced::Alignment::Center);

        // ─────────────────────────────────────────────────────────────────
        // MAIN INPUT BAR: Glassmorphism container (always visible)
        // ─────────────────────────────────────────────────────────────────

        let input_bar_content = row![
            left_buttons,
            Space::new().width(Length::Fixed(8.0)),
            input_field,
            Space::new().width(Length::Fixed(8.0)),
            right_buttons,
        ]
        .padding([6, 10])
        .align_y(iced::Alignment::Center);

        let input_bar = container(input_bar_content)
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(Color {
                    a: 0.6,
                    ..pal.surface_raised
                })),
                border: Border {
                    color: Color {
                        a: 0.4,
                        ..pal.border
                    },
                    width: 1.0,
                    radius: 20.0.into(),
                },
                ..Default::default()
            });

        // Outer container with padding - adjust left padding based on sidebar width
        let left_pad = if sidebar_width > 1.0 { sidebar_width } else { 0.0 };
        container(input_bar)
            .padding(iced::padding::Padding {
                top: 12.0,
                right: 16.0,
                bottom: 12.0,
                left: left_pad.max(0.0) + 16.0,
            })
            .width(Length::Fill)
            .style(transparent_style())
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
                    SettingsPage::Main => Space::new().into(),
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
                Space::new().width(Length::Fixed(submenu_slide)),
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
                Space::new().width(Length::Fixed(20.0)),
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

        // Calculate slide-up offset for content (matches backdrop animation)
        // Using cubic ease-out: 1.0 - (1.0 - t)^3
        let t = progress.min(1.0);
        let eased_progress = 1.0 - (1.0 - t).powi(3);
        let content_offset = (1.0 - eased_progress) * 30.0; // Slides up 30px

        // Close button at top-right (positioned absolutely via stack)
        let close_button = button(
            container(
                bootstrap::x_lg()
                    .size(18)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.text)
                    })
            )
            .width(Length::Fixed(40.0))
            .height(Length::Fixed(40.0))
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
        )
        .on_press(Message::CloseSettings)
        .padding(0)
        .style(move |_theme, status| {
            let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: Some(Background::Color(Color {
                    a: if is_hovered { 0.3 } else { 0.15 },
                    ..pal.surface_raised
                })),
                border: Border {
                    radius: 20.0.into(),
                    width: 1.0,
                    color: Color {
                        a: if is_hovered { 0.5 } else { 0.3 },
                        ..pal.border
                    },
                },
                text_color: pal.text,
                ..Default::default()
            }
        });

        // Position close button in top-right corner (floats above content)
        let close_button_layer = container(
            column![
                row![
                    Space::new().width(Length::Fill),
                    container(close_button).padding([16, 20]),
                ]
            ]
        )
        .width(Length::Fill)
        .height(Length::Shrink);

        // Content appears after initial animation
        let content: Element<'_, Message> = if progress > 0.15 {
            let content_opacity = (progress - 0.15).min(0.85) / 0.85;
            
            // Settings content with slide-up animation
            let settings_content = container(
                column![
                    Space::new().height(Length::Fixed(content_offset)),
                    styled_content,
                ]
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                text_color: Some(Color {
                    a: content_opacity,
                    ..pal.text
                }),
                ..Default::default()
            });

            // Close button overlay with same fade
            let close_overlay = container(close_button_layer)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| container::Style {
                    text_color: Some(Color {
                        a: content_opacity,
                        ..pal.text
                    }),
                    ..Default::default()
                });

            // Stack content and close button (close button on top)
            stack(vec![
                settings_content.into(),
                close_overlay.into(),
            ]).into()
        } else {
            Space::new().into()
        };

        // Stack: animated backdrop + content with floating close button
        stack(vec![liquid_bg.into(), content]).into()
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
            bootstrap::cpu(),
            "Provider & Model",
            "AI provider and model",
            Message::SettingsNavigate(SettingsPage::Provider),
            pal,
        );

        let behavior_btn = self.category_button(
            bootstrap::sliders(),
            "Behavior",
            "AI behavior settings",
            Message::SettingsNavigate(SettingsPage::Behavior),
            pal,
        );

        let appearance_btn = self.category_button(
            bootstrap::palette(),
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
                Space::new().height(Length::Fixed(16.0)),
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
        icon: iced::widget::Text<'static>,
        title: &'static str,
        subtitle: &'static str,
        on_press: Message,
        pal: PaletteColors,
    ) -> Element<'static, Message> {
        let icon_text = icon
            .size(20)
            .style(move |_| iced::widget::text::Style {
                color: Some(pal.accent),
            });

        let arrow = bootstrap::chevron_right()
            .size(16)
            .style(move |_| iced::widget::text::Style {
                color: Some(pal.muted),
            });

        let content = row![
            icon_text,
            Space::new().width(Length::Fixed(12.0)),
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
            Space::new().width(Length::Fill),
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

        // Provider dropdown
        let provider_content = column![
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
        ]
        .spacing(8)
        .width(Length::Fill);

        // Endpoint URL selector (shown above model)
        let endpoint_selector_content: Element<'a, Message> = if form.is_zai_provider() {
            // Z.AI provider: show endpoint dropdown with predefined options
            let mut endpoint_options = form.endpoint_options.clone();
            // Add "Custom" option if not already present
            if !endpoint_options.contains(&"Custom".to_string()) {
                endpoint_options.push("Custom".to_string());
            }

            let endpoint_selector = pick_list(
                endpoint_options,
                Some(form.endpoint_name.clone()),
                Message::ConfigEndpointChanged,
            );

            // Show text input only when Custom is selected
            if form.endpoint_name == "Custom" {
                column![
                    text("Endpoint URL")
                        .size(12)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        }),
                    endpoint_selector,
                    Space::new().height(Length::Fixed(4.0)),
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
                    text("Endpoint URL")
                        .size(12)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        }),
                    endpoint_selector,
                    Space::new().height(Length::Fixed(4.0)),
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
            column![
                text("Endpoint URL")
                    .size(12)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    }),
                text_input("https://api.example.com/v1", &form.api_url)
                    .on_input(Message::ConfigApiUrlChanged)
                    .padding(8)
                    .style(input_style(pal)),
            ]
            .spacing(8)
            .into()
        };

        // Model selector
        let model_content = column![
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
                    Space::new().width(Length::Fill),
                    bootstrap::chevron_right()
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
        ]
        .spacing(8)
        .width(Length::Fill);

        // API Key field
        let api_key_content = column![
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
        ]
        .spacing(8)
        .width(Length::Fill);

        // Thinking toggle
        let thinking_content = column![
            row![
                checkbox(form.thinking_enabled)
                    .on_toggle(Message::ConfigThinkingToggled)
                    .size(16)
                    .style(move |_theme, _status| {
                        iced::widget::checkbox::Style {
                            background: Background::Color(Color {
                                a: 0.1,
                                ..pal.accent
                            }),
                            border: Border {
                                radius: 4.0.into(),
                                width: 1.0,
                                color: Color {
                                    a: 0.3,
                                    ..pal.accent
                                },
                            },
                            icon_color: pal.accent,
                            text_color: Some(pal.text),
                        }
                    }),
                text("Enable thinking mode")
                    .size(14)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.text)
                    }),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(8),
            text("Note: Requires reasoning models (OpenAI o1/o3, Claude with thinking)")
                .size(11)
                .style(move |_| iced::widget::text::Style {
                    color: Some(pal.muted)
                }),
        ]
        .spacing(4);

        let base_content = column![
            provider_content,
            Space::new().height(Length::Fixed(12.0)),
            endpoint_selector_content,
            Space::new().height(Length::Fixed(16.0)),
            model_content,
            Space::new().height(Length::Fixed(16.0)),
            api_key_content,
            Space::new().height(Length::Fixed(16.0)),
            thinking_content,
            Space::new().height(Length::Fixed(12.0)),
        ]
        .spacing(0)
        .width(Length::Fill);

        let content = container(base_content)
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
            Space::new().height(Length::Fixed(12.0)),
            content,
            Space::new().height(Length::Fixed(12.0)),
            row![save_btn, Space::new().width(Length::Fixed(12.0)), status]
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
                Space::new().height(Length::Fixed(12.0)),
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
                Space::new().height(Length::Fixed(12.0)),
                row![
                    text("Enable Streaming")
                        .size(14)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.text)
                        }),
                    Space::new().width(Length::Fill),
                    iced::widget::toggler(form.streaming_enabled)
                        .on_toggle(Message::ConfigStreamingToggled)
                        .width(Length::Shrink)
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center),
                Space::new().height(Length::Fill),
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
            Space::new().height(Length::Fixed(12.0)),
            content,
            Space::new().height(Length::Fixed(12.0)),
            row![save_btn, Space::new().width(Length::Fixed(12.0)), status]
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

        // Theme mode selection (Light vs Dark)
        let light_dark_options = vec!["Light".to_string(), "Dark".to_string()];
        let theme_selector = row![
            column![
                text("Theme Mode").size(14).style(move |_| {
                    iced::widget::text::Style {
                        color: Some(pal.text),
                    }
                }),
                text("Choose your preferred color scheme")
                    .size(12)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    }),
            ],
            Space::new().width(Length::Fill),
            pick_list(
                light_dark_options,
                Some(match self.theme_mode {
                    ThemeMode::Light => "Light".to_string(),
                    _ => "Dark".to_string(),
                }),
                Message::ThemeModeChanged,
            )
            .padding([8, 12])
            .style(move |_theme, _status| iced::widget::pick_list::Style {
                background: Background::Color(pal.surface),
                text_color: pal.text,
                placeholder_color: pal.muted,
                border: Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: pal.border,
                },
                handle_color: pal.accent,
            })
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center);

        // Dark theme submenu (only show when Dark/Black is selected)
        let dark_submenu = if matches!(self.theme_mode, ThemeMode::Dark | ThemeMode::Black) {
            let dark_black_options = vec!["Dark".to_string(), "Black".to_string()];
            let submenu = row![
                column![
                    text("Dark Theme Style").size(14).style(move |_| {
                        iced::widget::text::Style {
                            color: Some(pal.text),
                        }
                    }),
                    text("Choose between dark or pure black background")
                        .size(12)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        }),
                ],
                Space::new().width(Length::Fill),
                pick_list(
                    dark_black_options,
                    Some(match self.theme_mode {
                        ThemeMode::Black => "Black".to_string(),
                        _ => "Dark".to_string(),
                    }),
                    Message::ThemeSubmenuChanged,
                )
                .padding([8, 12])
                .style(move |_theme, _status| iced::widget::pick_list::Style {
                    background: Background::Color(pal.surface),
                    text_color: pal.text,
                    placeholder_color: pal.muted,
                    border: Border {
                        radius: 8.0.into(),
                        width: 1.0,
                        color: pal.border,
                    },
                    handle_color: pal.accent,
                })
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center);
            
            Some(submenu)
        } else {
            None
        };

        // Living background toggle
        let living_bg_toggle = row![
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
            Space::new().width(Length::Fill),
            iced::widget::toggler(form.living_background_enabled)
                .on_toggle(Message::ConfigLivingBackgroundToggled)
                .width(Length::Shrink)
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center);

        // Build the content column
        let mut content_col = column![
            text("Visual Settings")
                .size(14)
                .style(move |_| iced::widget::text::Style {
                    color: Some(pal.muted)
                }),
            Space::new().height(Length::Fixed(12.0)),
            theme_selector,
        ];

        // Add dark submenu if visible
        if let Some(submenu) = dark_submenu {
            content_col = content_col.push(Space::new().height(Length::Fixed(16.0)));
            content_col = content_col.push(submenu);
        }

        // Add living background toggle
        content_col = content_col.push(Space::new().height(Length::Fixed(16.0)));
        content_col = content_col.push(living_bg_toggle);
        content_col = content_col.push(Space::new().height(Length::Fill));

        let content = container(content_col)
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
            Space::new().height(Length::Fixed(12.0)),
            content,
            Space::new().height(Length::Fixed(12.0)),
            row![save_btn, Space::new().width(Length::Fixed(12.0)), status]
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

        // Create the custom model input section (shown for all providers when not loading)
        let custom_model_section: Element<'_, Message> = {
            let draft_value = self.custom_model_draft.clone();
            let can_add = !self.custom_model_draft.trim().is_empty();
            
            let input_field = text_input("Enter custom model name...", &draft_value)
                .on_input(Message::CustomModelDraftChanged)
                .padding(8)
                .style(input_style(pal));
            
            let add_btn = button(
                row![
                    bootstrap::plus_lg()
                        .size(14)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(if can_add { pal.accent } else { pal.muted })
                        }),
                    Space::new().width(Length::Fixed(6.0)),
                    text("Add")
                        .size(13)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(if can_add { pal.text } else { pal.muted })
                        }),
                ]
                .align_y(iced::Alignment::Center)
            )
            .on_press_maybe(if can_add { Some(Message::AddCustomModel) } else { None })
            .padding([8, 12])
            .style(move |_theme, status| {
                let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                let is_disabled = !can_add;
                iced::widget::button::Style {
                    background: Some(Background::Color(if is_disabled {
                        Color { a: 0.05, ..pal.text }
                    } else if is_hovered {
                        Color { a: 0.2, ..pal.accent }
                    } else {
                        Color { a: 0.1, ..pal.accent }
                    })),
                    border: Border {
                        radius: 8.0.into(),
                        width: 1.0,
                        color: if is_disabled {
                            Color { a: 0.1, ..pal.text }
                        } else {
                            Color { a: 0.2, ..pal.accent }
                        },
                    },
                    text_color: if is_disabled { pal.muted } else { pal.text },
                    ..Default::default()
                }
            });
            
            column![
                container(
                    row![
                        bootstrap::pencil().size(12).style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        }),
                        Space::new().width(Length::Fixed(6.0)),
                        text("Custom Model")
                            .size(12)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(pal.muted)
                            }),
                    ]
                    .align_y(iced::Alignment::Center)
                )
                .padding([8, 0]),
                row![
                    input_field,
                    Space::new().width(Length::Fixed(8.0)),
                    add_btn,
                ]
                .align_y(iced::Alignment::Center)
            ]
            .spacing(4)
            .width(Length::Fill)
            .into()
        };

        let content_items: Element<'_, Message> = if self.models_loading {
            // Show loading spinner with custom animation
            let spinner = Canvas::new(LoadingSpinner::new(SpinnerState {
                tick: self.spinner_state.tick,
                spinner_type: SpinnerType::Ring,
                size: 20.0,
                color: pal.accent,
                accent_color: Color {
                    r: pal.accent.r * 0.5,
                    g: pal.accent.g * 0.5,
                    b: pal.accent.b * 0.5,
                    a: pal.accent.a,
                },
            }))
            .width(Length::Fixed(40.0))
            .height(Length::Fixed(40.0));

            column![
                Space::new().height(Length::Fixed(40.0)),
                spinner,
                Space::new().height(Length::Fixed(16.0)),
                text("Fetching models...")
                    .size(16)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    }),
                Space::new().height(Length::Fill),
            ]
            .align_x(iced::Alignment::Center)
            .width(Length::Fill)
            .into()
        } else if self.model_list.is_empty() {
            // No models from provider - show custom model input prominently
            column![
                Space::new().height(Length::Fixed(20.0)),
                text("No models fetched from provider")
                    .size(14)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    }),
                Space::new().height(Length::Fixed(8.0)),
                text("Enter a custom model name below:")
                    .size(12)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(pal.muted)
                    }),
                Space::new().height(Length::Fixed(16.0)),
                custom_model_section,
                Space::new().height(Length::Fill),
            ]
            .align_x(iced::Alignment::Center)
            .width(Length::Fill)
            .into()
        } else {
            // Check if the list contains error/warning messages (starts with ⚠️)
            let has_errors = self.model_list.iter().any(|m| m.starts_with("⚠️"));
            
            if has_errors {
                // Display error messages as non-selectable text
                let mut error_col = column![].spacing(8).width(Length::Fill);
                for message in &self.model_list {
                    if message.starts_with("⚠️") {
                        // Create an owned String from the trimmed message to avoid lifetime issues
                        let error_text = message.trim_start_matches("⚠️").trim().to_string();
                        error_col = error_col.push(
                            container(
                                row![
                                    bootstrap::exclamation_triangle_fill()
                                        .size(16)
                                        .style(move |_| iced::widget::text::Style {
                                            color: Some(pal.danger)
                                        }),
                                    Space::new().width(Length::Fixed(8.0)),
                                    text(error_text)
                                        .size(13)
                                        .style(move |_| iced::widget::text::Style {
                                            color: Some(pal.danger)
                                        }),
                                ]
                                .align_y(iced::Alignment::Center)
                            )
                            .padding([12, 16])
                            .width(Length::Fill)
                            .style(move |_| container::Style {
                                background: Some(Background::Color(Color {
                                    a: 0.1,
                                    ..pal.danger
                                })),
                                border: Border {
                                    radius: 8.0.into(),
                                    width: 1.0,
                                    color: Color {
                                        a: 0.2,
                                        ..pal.danger
                                    },
                                },
                                ..Default::default()
                            })
                        );
                    }
                }
                
                // Add helpful text and custom model section
                error_col = error_col.push(Space::new().height(Length::Fixed(8.0)));
                error_col = error_col.push(
                    text("Check your provider settings or enter a custom model below:")
                        .size(12)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(pal.muted)
                        })
                );
                error_col = error_col.push(Space::new().height(Length::Fixed(12.0)));
                error_col = error_col.push(custom_model_section);
                
                column![
                    Space::new().height(Length::Fixed(20.0)),
                    error_col,
                    Space::new().height(Length::Fill),
                ]
                .align_x(iced::Alignment::Center)
                .width(Length::Fill)
                .into()
            } else {
                // Show model list as buttons with custom model input at the bottom
                let mut model_col = column![].spacing(4).width(Length::Fill);
                for model in &self.model_list {
                    let model_name = model.clone();
                    let model_display = model.clone();
                    let is_selected = model == &self.config_form.model;
                    let model_btn = button(
                        row![
                            bootstrap::check_lg()
                                .size(14)
                                .style(move |_| iced::widget::text::Style {
                                    color: Some(if is_selected {
                                        pal.accent
                                    } else {
                                        Color::TRANSPARENT
                                    })
                                }),
                            Space::new().width(Length::Fixed(8.0)),
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
                
                // Add separator and custom model section
                model_col = model_col.push(Space::new().height(Length::Fixed(12.0)));
                model_col = model_col.push(
                    container(Space::new().height(Length::Fixed(1.0)))
                        .width(Length::Fill)
                        .style(move |_| container::Style {
                            background: Some(Background::Color(Color {
                                a: 0.15,
                                ..pal.text
                            })),
                            ..Default::default()
                        })
                );
                model_col = model_col.push(Space::new().height(Length::Fixed(12.0)));
                model_col = model_col.push(custom_model_section);
                
                iced::widget::scrollable(model_col)
                    .height(Length::Fill)
                    .into()
            }
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

        column![header, Space::new().height(Length::Fixed(12.0)), content,]
            .spacing(4)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

fn main() -> iced::Result {
    fn get_theme(app: &App) -> iced::Theme {
        app_theme_with_mode(app.theme_mode)
    }
    
    iced::application(App::init, App::update, App::view)
        .title("Arula Desktop")
        .subscription(App::subscription)
        .theme(get_theme)
        .font(iced_fonts::BOOTSTRAP_FONT_BYTES)
        .run()
}