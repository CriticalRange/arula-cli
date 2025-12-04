use arula_core::utils::config::{AiConfig, Config};
use arula_core::{AgentBackend, SessionConfig, SessionRunner, StreamEvent};
use chrono::Utc;
use futures::StreamExt;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{
    button, column, container, pick_list, row, scrollable, text, text_input, Space,
};
use iced::{theme, Background, Border, Color, Element, Length, Shadow, Subscription, Task, Theme};
use tokio::runtime::Runtime;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

#[derive(Clone, Copy)]
struct PaletteColors {
    background: Color,
    surface: Color,
    surface_raised: Color,
    border: Color,
    text: Color,
    muted: Color,
    accent: Color,
    accent_soft: Color,
    success: Color,
    danger: Color,
}

fn palette() -> PaletteColors {
    PaletteColors {
        background: Color::from_rgb8(12, 14, 18),
        surface: Color::from_rgb8(20, 24, 31),
        surface_raised: Color::from_rgb8(28, 33, 42),
        border: Color::from_rgb8(45, 54, 67),
        text: Color::from_rgb8(230, 235, 245),
        muted: Color::from_rgb8(156, 166, 182),
        accent: Color::from_rgb8(64, 201, 182),
        accent_soft: Color::from_rgb8(42, 125, 118),
        success: Color::from_rgb8(116, 201, 144),
        danger: Color::from_rgb8(229, 120, 114),
    }
}

fn app_theme() -> Theme {
    let p = palette();
    Theme::custom(
        "Arula Midnight".to_string(),
        theme::Palette {
            background: p.background,
            text: p.text,
            primary: p.accent,
            success: p.success,
            danger: p.danger,
        },
    )
}

fn card_style(palette: PaletteColors) -> impl Fn(&Theme) -> container::Style + Clone {
    move |_| container::Style {
        text_color: Some(palette.text),
        background: Some(Background::Color(palette.surface)),
        border: Border {
            color: palette.border,
            width: 1.0,
            radius: 12.0.into(),
        },
        shadow: Shadow::default(),
    }
}

fn input_style(
    palette: PaletteColors,
) -> impl Fn(&Theme, text_input::Status) -> text_input::Style + Clone {
    move |_, status| {
        let is_focused = matches!(
            status,
            text_input::Status::Focused | text_input::Status::Hovered
        );
        let border_color = if is_focused {
            palette.accent
        } else {
            palette.border
        };

        text_input::Style {
            background: Background::Color(palette.surface_raised),
            border: Border {
                color: border_color,
                width: if is_focused { 1.5 } else { 1.0 },
                radius: 8.0.into(),
            },
            icon: palette.muted,
            placeholder: palette.muted,
            value: palette.text,
            selection: palette.accent,
        }
    }
}

fn primary_button_style(
    palette: PaletteColors,
) -> impl Fn(&Theme, button::Status) -> button::Style + Clone {
    move |_, status| {
        let bg = match status {
            button::Status::Hovered => palette.accent_soft,
            button::Status::Pressed => Color {
                a: 1.0,
                ..palette.accent_soft
            },
            button::Status::Disabled => Color {
                a: 0.35,
                ..palette.accent
            },
            _ => palette.accent,
        };

        let text_color = if matches!(status, button::Status::Disabled) {
            Color {
                a: 0.6,
                ..palette.background
            }
        } else {
            palette.background
        };

        button::Style {
            background: Some(Background::Color(bg)),
            text_color,
            border: Border {
                color: palette.accent,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: Shadow::default(),
        }
    }
}

fn pill_button_style(
    palette: PaletteColors,
    active: bool,
    active_color: Color,
    hover_color: Color,
) -> impl Fn(&Theme, button::Status) -> button::Style + Clone {
    move |_, status| {
        let (bg, border_color, text_color) = if active {
            let hover = Color {
                a: 0.9,
                ..hover_color
            };
            let base = Color {
                a: 0.85,
                ..active_color
            };
            match status {
                button::Status::Hovered => (hover, active_color, palette.background),
                button::Status::Pressed => (hover_color, active_color, palette.background),
                _ => (base, active_color, palette.background),
            }
        } else {
            let hover = palette.surface_raised;
            match status {
                button::Status::Hovered => (hover, palette.border, palette.text),
                button::Status::Pressed => (palette.surface, palette.border, palette.text),
                _ => (palette.surface, palette.border, palette.text),
            }
        };

        button::Style {
            background: Some(Background::Color(bg)),
            text_color,
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 18.0.into(),
            },
            shadow: Shadow::default(),
        }
    }
}

#[derive(Debug, Clone)]
enum UiEvent {
    StreamStarted(Uuid),
    Token(Uuid, String, bool),
    StreamFinished(Uuid),
    StreamErrored(Uuid, String),
}

struct Dispatcher {
    runtime: Runtime,
    events: broadcast::Sender<UiEvent>,
    runner: SessionRunner<AgentBackend>,
}

impl Dispatcher {
    fn new(config: &Config) -> anyhow::Result<Self> {
        let backend = AgentBackend::new(config, String::new())?;
        let runtime = Runtime::new()?;
        let (events, _) = broadcast::channel(128);
        let runner = SessionRunner::new(backend);
        Ok(Self {
            runtime,
            events,
            runner,
        })
    }

    fn update_backend(&mut self, config: &Config) -> anyhow::Result<()> {
        let backend = AgentBackend::new(config, String::new())?;
        self.runner = SessionRunner::new(backend);
        Ok(())
    }

    fn start_stream(
        &self,
        session_id: Uuid,
        prompt: String,
        session_config: SessionConfig,
    ) -> anyhow::Result<()> {
        let tx = self.events.clone();
        let runner = self.runner.clone();
        self.runtime.spawn(async move {
            let _ = tx.send(UiEvent::StreamStarted(session_id));
            match runner.stream_session(prompt, None, session_config) {
                Ok(mut stream) => {
                    while let Some(event) = stream.next().await {
                        match event {
                            StreamEvent::Start { .. } => {}
                            StreamEvent::Text { text } => {
                                let _ = tx.send(UiEvent::Token(session_id, text, false));
                            }
                            StreamEvent::Reasoning { text } => {
                                let _ = tx.send(UiEvent::Token(
                                    session_id,
                                    format!("[thinking] {text}"),
                                    false,
                                ));
                            }
                            StreamEvent::ToolCall {
                                id,
                                name,
                                arguments,
                            } => {
                                let _ = tx.send(UiEvent::Token(
                                    session_id,
                                    format!("[tool:{name}] {arguments} ({id})"),
                                    false,
                                ));
                            }
                            StreamEvent::ToolResult {
                                tool_call_id,
                                result,
                            } => {
                                let _ = tx.send(UiEvent::Token(
                                    session_id,
                                    format!("[tool-result:{tool_call_id}] {:?}", result),
                                    false,
                                ));
                            }
                            StreamEvent::Finished => {
                                let _ = tx.send(UiEvent::Token(session_id, String::new(), true));
                                let _ = tx.send(UiEvent::StreamFinished(session_id));
                                break;
                            }
                            StreamEvent::Error(err) => {
                                let _ = tx.send(UiEvent::StreamErrored(session_id, err));
                                break;
                            }
                        }
                    }
                }
                Err(err) => {
                    let _ = tx.send(UiEvent::StreamErrored(session_id, err.to_string()));
                }
            }
        });
        Ok(())
    }

    fn subscription(&self) -> Subscription<UiEvent> {
        let rx = self.events.subscribe();
        iced::Subscription::run_with_id("dispatcher-stream", {
            let rx_stream = BroadcastStream::new(rx);
            iced::futures::stream::unfold(rx_stream, |mut stream| async {
                while let Some(next) = stream.next().await {
                    match next {
                        Ok(ev) => return Some((ev, stream)),
                        Err(_) => continue,
                    }
                }
                None
            })
        })
    }
}

#[derive(Debug, Clone)]
struct MessageEntry {
    role: String,
    content: String,
    timestamp: String,
}

#[derive(Debug, Clone)]
struct Session {
    id: Uuid,
    messages: Vec<MessageEntry>,
    is_streaming: bool,
}

#[derive(Debug, Clone)]
struct ConfigForm {
    provider: String,
    model: String,
    api_url: String,
    api_key: String,
    thinking_enabled: bool,
    web_search_enabled: bool,
    ollama_tools_enabled: bool,
    provider_options: Vec<String>,
    status: Option<String>,
}

impl ConfigForm {
    fn with_provider_options(
        config: &Config,
        provider: String,
        provider_options: Vec<String>,
    ) -> Self {
        let defaults = AiConfig::get_provider_defaults(&provider);
        let provider_config = config.providers.get(&provider);

        let model = provider_config
            .map(|p| p.model.clone())
            .unwrap_or(defaults.model);
        let api_url = provider_config
            .and_then(|p| p.api_url.clone())
            .unwrap_or(defaults.api_url);
        let api_key = provider_config
            .map(|p| p.api_key.clone())
            .unwrap_or(defaults.api_key);
        let thinking_enabled = provider_config
            .and_then(|p| p.thinking_enabled)
            .unwrap_or(false);
        let web_search_enabled = provider_config
            .and_then(|p| p.web_search_enabled)
            .unwrap_or(false);
        let ollama_tools_enabled = provider_config
            .and_then(|p| p.tools_enabled)
            .unwrap_or(false);

        Self {
            provider,
            model,
            api_url,
            api_key,
            thinking_enabled,
            web_search_enabled,
            ollama_tools_enabled,
            provider_options,
            status: None,
        }
    }

    fn from_config(config: &Config) -> Self {
        let provider_options = collect_provider_options(config);
        Self::with_provider_options(config, config.active_provider.clone(), provider_options)
    }

    fn api_url_editable(&self) -> bool {
        matches!(self.provider.to_lowercase().as_str(), "custom" | "ollama")
    }

    fn search_provider_label(&self) -> &'static str {
        if self.web_search_enabled && self.provider.to_lowercase().contains("z.ai") {
            "Z.AI"
        } else {
            "DuckDuckGo"
        }
    }
}

struct App {
    dispatcher: Dispatcher,
    sessions: Vec<Session>,
    current: usize,
    draft: String,
    config: Config,
    config_form: ConfigForm,
}

#[derive(Debug, Clone)]
enum Message {
    DraftChanged(String),
    SendPrompt,
    Received(UiEvent),
    NewTab,
    ConfigProviderChanged(String),
    ConfigModelChanged(String),
    ConfigApiUrlChanged(String),
    ConfigApiKeyChanged(String),
    ConfigThinkingToggled(bool),
    ConfigWebSearchToggled(bool),
    ConfigOllamaToolsToggled(bool),
    SaveConfig,
}

impl App {
    fn init() -> (Self, Task<Message>) {
        let config = Config::load_or_default().expect("configuration");
        let dispatcher = Dispatcher::new(&config).expect("dispatcher");
        let config_form = ConfigForm::from_config(&config);
        let session = Session {
            id: Uuid::new_v4(),
            messages: Vec::new(),
            is_streaming: false,
        };

        (
            Self {
                dispatcher,
                sessions: vec![session],
                current: 0,
                draft: String::new(),
                config,
                config_form,
            },
            Task::none(),
        )
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
                    session.messages.push(MessageEntry {
                        role: "User".into(),
                        content: prompt.clone(),
                        timestamp: Utc::now().to_rfc3339(),
                    });
                    session.is_streaming = true;
                    let sid = session.id;
                    let session_config = SessionConfig {
                        system_prompt: String::new(),
                        model: self.config.get_model(),
                        max_tokens: 1024,
                        temperature: 0.7,
                    };
                    if let Err(err) = self.dispatcher.start_stream(sid, prompt, session_config) {
                        eprintln!("dispatch error: {err}");
                        session.is_streaming = false;
                    }
                }
            }
            Message::Received(ev) => match ev {
                UiEvent::StreamStarted(id) => {
                    if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                        s.is_streaming = true;
                    }
                }
                UiEvent::Token(id, delta, is_final) => {
                    if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                        if let Some(last) = s.messages.last_mut() {
                            if last.role == "Arula" {
                                last.content.push_str(&delta);
                            } else {
                                s.messages.push(MessageEntry {
                                    role: "Arula".into(),
                                    content: delta,
                                    timestamp: Utc::now().to_rfc3339(),
                                });
                            }
                        } else {
                            s.messages.push(MessageEntry {
                                role: "Arula".into(),
                                content: delta,
                                timestamp: Utc::now().to_rfc3339(),
                            });
                        }
                        if is_final {
                            s.is_streaming = false;
                        }
                    }
                }
                UiEvent::StreamFinished(id) => {
                    if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                        s.is_streaming = false;
                    }
                }
                UiEvent::StreamErrored(id, err) => {
                    eprintln!("stream error {id}: {err}");
                    if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                        s.is_streaming = false;
                    }
                }
            },
            Message::NewTab => {
                self.sessions.push(Session {
                    id: Uuid::new_v4(),
                    messages: Vec::new(),
                    is_streaming: false,
                });
                self.current = self.sessions.len() - 1;
            }
            Message::ConfigProviderChanged(provider) => {
                let options = collect_provider_options(&self.config);
                self.config_form =
                    ConfigForm::with_provider_options(&self.config, provider, options);
            }
            Message::ConfigModelChanged(model) => {
                self.config_form.model = model;
            }
            Message::ConfigApiUrlChanged(url) => {
                if self.config_form.api_url_editable() {
                    self.config_form.api_url = url;
                    self.config_form.status = None;
                } else {
                    self.config_form.status =
                        Some("API URL is managed by this provider".to_string());
                }
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
            Message::SaveConfig => {
                self.apply_config_changes();
            }
        }
        Task::none()
    }

    fn apply_config_changes(&mut self) {
        let selected_provider = self.config_form.provider.clone();
        if self.config.active_provider != selected_provider {
            if let Err(err) = self.config.switch_provider(&selected_provider) {
                self.config_form.status = Some(format!("Failed to switch provider: {err}"));
                return;
            }
        }

        self.config.set_model(&self.config_form.model);
        if self.config_form.api_url_editable() {
            self.config.set_api_url(&self.config_form.api_url);
        }
        self.config.set_api_key(&self.config_form.api_key);

        if let Some(active) = self.config.get_active_provider_config_mut() {
            active.thinking_enabled = Some(self.config_form.thinking_enabled);
            active.web_search_enabled = Some(self.config_form.web_search_enabled);
            active.tools_enabled = Some(self.config_form.ollama_tools_enabled);
        }

        match self.config.save() {
            Ok(_) => {
                if let Err(err) = self.dispatcher.update_backend(&self.config) {
                    self.config_form.status =
                        Some(format!("Saved, but backend failed to refresh: {err}"));
                    return;
                }
                self.config_form = ConfigForm::from_config(&self.config);
                self.config_form.status = Some("Settings saved".to_string());
            }
            Err(err) => {
                self.config_form.status = Some(format!("Failed to save settings: {err}"));
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        self.dispatcher.subscription().map(Message::Received)
    }

    fn view(&self) -> Element<'_, Message> {
        let palette = palette();
        let layout = row![
            self.left_rail(palette),
            column![
                self.header(palette),
                row![self.config_panel(palette), self.chat_panel(palette)]
                    .spacing(16)
                    .height(Length::Fill)
            ]
            .spacing(14)
            .padding([12, 14])
            .width(Length::Fill),
        ]
        .spacing(14);

        container(layout)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(12)
            .style(move |_| container::Style {
                text_color: Some(palette.text),
                background: Some(Background::Color(palette.background)),
                border: Border {
                    color: palette.background,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                shadow: Shadow::default(),
            })
            .into()
    }

    fn left_rail(&self, palette: PaletteColors) -> Element<'_, Message> {
        let provider_code = self
            .config
            .active_provider
            .chars()
            .take(3)
            .collect::<String>()
            .to_uppercase();

        let logo = container(
            text("A")
                .size(16)
                .style(move |_| iced::widget::text::Style {
                    color: Some(palette.accent),
                }),
        )
        .width(Length::Fixed(40.0))
        .height(Length::Fixed(40.0))
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .style(move |_| container::Style {
            text_color: Some(palette.accent),
            background: Some(Background::Color(palette.surface_raised)),
            border: Border {
                color: palette.border,
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow: Shadow::default(),
        });

        let status = column![
            text(provider_code)
                .size(12)
                .style(move |_| iced::widget::text::Style {
                    color: Some(palette.muted),
                }),
            container(Space::with_width(Length::Fixed(8.0)).height(Length::Fixed(8.0)))
                .style(move |_| container::Style {
                    text_color: Some(palette.text),
                    background: Some(Background::Color(palette.accent)),
                    border: Border {
                        color: palette.accent_soft,
                        width: 1.0,
                        radius: 12.0.into(),
                    },
                    shadow: Shadow::default(),
                })
                .width(Length::Fixed(12.0))
                .height(Length::Fixed(12.0)),
        ]
        .spacing(8)
        .align_x(iced::Alignment::Center);

        container(
            column![logo, Space::with_height(Length::Fill), status]
                .spacing(24)
                .align_x(iced::Alignment::Center),
        )
        .padding([18, 12])
        .width(Length::Fixed(76.0))
        .style(move |_| container::Style {
            text_color: Some(palette.text),
            background: Some(Background::Color(palette.surface)),
            border: Border {
                color: palette.border,
                width: 1.0,
                radius: 14.0.into(),
            },
            shadow: Shadow::default(),
        })
        .into()
    }

    fn header(&self, palette: PaletteColors) -> Element<'_, Message> {
        let badge = container(text(self.config.active_provider.clone()).size(14).style(
            move |_| iced::widget::text::Style {
                color: Some(palette.accent),
            },
        ))
        .padding([6, 12])
        .style(move |_| container::Style {
            text_color: Some(palette.accent),
            background: Some(Background::Color(palette.surface_raised)),
            border: Border {
                color: palette.border,
                width: 1.0,
                radius: 20.0.into(),
            },
            shadow: Shadow::default(),
        });

        let headline = column![
            text("Arula Desktop")
                .size(24)
                .style(move |_| iced::widget::text::Style {
                    color: Some(palette.text),
                }),
            text("Chat and provider controls in one place")
                .size(14)
                .style(move |_| iced::widget::text::Style {
                    color: Some(palette.muted),
                }),
        ]
        .spacing(4);

        container(
            row![
                row![headline, badge]
                    .spacing(12)
                    .align_y(iced::Alignment::Center),
                Space::with_width(Length::Fill),
                button("New tab")
                    .on_press(Message::NewTab)
                    .padding([8, 12])
                    .style(primary_button_style(palette)),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([8, 12])
        .style(move |_| container::Style {
            text_color: Some(palette.text),
            background: Some(Background::Color(palette.surface)),
            border: Border {
                color: palette.border,
                width: 1.0,
                radius: 14.0.into(),
            },
            shadow: Shadow::default(),
        })
        .into()
    }

    fn config_panel(&self, palette: PaletteColors) -> Element<'_, Message> {
        let form = &self.config_form;
        let card_style = card_style(palette);
        let input_style = input_style(palette);

        let provider_picker = pick_list(
            form.provider_options.clone(),
            Some(form.provider.clone()),
            Message::ConfigProviderChanged,
        );

        let api_url_field: Element<_> = text_input(
            if form.api_url_editable() {
                "API URL"
            } else {
                "API URL (managed by provider)"
            },
            &form.api_url,
        )
        .on_input(Message::ConfigApiUrlChanged)
        .padding(10)
        .style(input_style.clone())
        .into();

        let mut toggle_row = row![
            button("Thinking")
                .on_press(Message::ConfigThinkingToggled(!form.thinking_enabled))
                .padding([8, 12])
                .style(pill_button_style(
                    palette,
                    form.thinking_enabled,
                    palette.accent,
                    palette.accent_soft
                )),
            button(text(format!(
                "Web search ({})",
                form.search_provider_label()
            )))
            .on_press(Message::ConfigWebSearchToggled(!form.web_search_enabled))
            .padding([8, 12])
            .style(pill_button_style(
                palette,
                form.web_search_enabled,
                palette.success,
                palette.accent_soft
            )),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        if form.provider.to_lowercase() == "ollama" {
            toggle_row = toggle_row.push(
                button("Ollama tools")
                    .on_press(Message::ConfigOllamaToolsToggled(
                        !form.ollama_tools_enabled,
                    ))
                    .padding([8, 12])
                    .style(pill_button_style(
                        palette,
                        form.ollama_tools_enabled,
                        palette.muted,
                        palette.accent_soft,
                    )),
            );
        }

        let mut settings = column![
            text("Configuration")
                .size(18)
                .style(move |_| iced::widget::text::Style {
                    color: Some(palette.text),
                }),
            text("Pick your provider, key, and safety options.")
                .size(14)
                .style(move |_| iced::widget::text::Style {
                    color: Some(palette.muted),
                }),
            container(
                column![
                    text("Provider")
                        .size(14)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(palette.muted),
                        }),
                    provider_picker,
                    text("Choose a provider profile from your arula_core configuration.")
                        .size(12)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(palette.muted),
                        }),
                ]
                .spacing(6)
            )
            .padding(12)
            .style(card_style.clone()),
            container(
                column![
                    text("Model")
                        .size(14)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(palette.muted),
                        }),
                    text_input("Model", &form.model)
                        .on_input(Message::ConfigModelChanged)
                        .padding(10)
                        .style(input_style.clone()),
                    text("Update the model identifier for the active provider.")
                        .size(12)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(palette.muted),
                        }),
                ]
                .spacing(6)
            )
            .padding(12)
            .style(card_style.clone()),
            container(
                column![
                    text("API URL")
                        .size(14)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(palette.muted),
                        }),
                    api_url_field,
                    text("API Key")
                        .size(14)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(palette.muted),
                        }),
                    text_input("API key", &form.api_key)
                        .secure(true)
                        .on_input(Message::ConfigApiKeyChanged)
                        .padding(10)
                        .style(input_style.clone()),
                    text("Stored locally and passed to the backend session runner.")
                        .size(12)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(palette.muted),
                        }),
                ]
                .spacing(6)
            )
            .padding(12)
            .style(card_style.clone()),
        ]
        .spacing(12);

        settings = settings.push(container(toggle_row).padding(12).style(card_style.clone()));

        if let Some(status) = &form.status {
            let color = if status.to_lowercase().contains("fail") {
                palette.danger
            } else {
                palette.success
            };
            settings = settings.push(
                text(status)
                    .size(14)
                    .style(move |_| iced::widget::text::Style { color: Some(color) }),
            );
        }

        settings = settings.push(
            button("Save settings")
                .on_press(Message::SaveConfig)
                .padding([10, 14])
                .style(primary_button_style(palette)),
        );

        container(settings)
            .width(Length::Fixed(360.0))
            .style(card_style)
            .padding(14)
            .into()
    }

    fn chat_panel(&self, palette: PaletteColors) -> Element<'_, Message> {
        let session = &self.sessions[self.current];
        let messages: Element<_> = scrollable(
            column(
                session
                    .messages
                    .iter()
                    .map(|message| self.message_bubble(message, palette))
                    .collect::<Vec<_>>(),
            )
            .spacing(12),
        )
        .height(Length::Fill)
        .into();

        let input_style = input_style(palette);
        let composer = container(
            row![
                text_input("Message", &self.draft)
                    .on_input(Message::DraftChanged)
                    .padding(12)
                    .style(input_style)
                    .width(Length::Fill),
                button("Send")
                    .on_press(Message::SendPrompt)
                    .padding([10, 14])
                    .style(primary_button_style(palette)),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
        )
        .padding(12)
        .style(move |_| container::Style {
            text_color: Some(palette.text),
            background: Some(Background::Color(palette.surface_raised)),
            border: Border {
                color: palette.border,
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow: Shadow::default(),
        });

        let header = row![
            column![
                text("Conversation")
                    .size(16)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(palette.text),
                    }),
                text("Chat output with reasoning and tool calls")
                    .size(13)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(palette.muted),
                    }),
            ]
            .spacing(2),
            Space::with_width(Length::Fill),
            if session.is_streaming {
                text("Streaming…")
                    .size(13)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(palette.accent),
                    })
            } else {
                text("Idle")
                    .size(13)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(palette.muted),
                    })
            }
        ]
        .align_y(iced::Alignment::Center);

        let chat_column = column![header, messages, composer].spacing(12);

        container(chat_column)
            .padding(16)
            .style(card_style(palette))
            .into()
    }

    fn message_bubble<'a>(
        &'a self,
        message: &'a MessageEntry,
        palette: PaletteColors,
    ) -> Element<'a, Message> {
        let is_user = message.role.to_lowercase() == "user";
        let background = if is_user {
            palette.accent_soft
        } else {
            palette.surface_raised
        };
        let border_color = if is_user {
            palette.accent
        } else {
            palette.border
        };

        let avatar = container(
            text(if is_user { "U" } else { "A" })
                .size(12)
                .style(move |_| iced::widget::text::Style {
                    color: Some(palette.text),
                }),
        )
        .width(Length::Fixed(32.0))
        .height(Length::Fixed(32.0))
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .style(move |_| container::Style {
            text_color: Some(palette.text),
            background: Some(Background::Color(if is_user {
                palette.surface
            } else {
                palette.surface_raised
            })),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 16.0.into(),
            },
            shadow: Shadow::default(),
        });

        let bubble = container(
            column![
                text(&message.content)
                    .size(16)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(palette.text),
                    }),
                text(&message.timestamp)
                    .size(12)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(palette.muted),
                    }),
            ]
            .spacing(6),
        )
        .padding([10, 12])
        .style(move |_| container::Style {
            text_color: Some(palette.text),
            background: Some(Background::Color(background)),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 14.0.into(),
            },
            shadow: Shadow::default(),
        });

        let row = if is_user {
            row![Space::with_width(Length::Fill), bubble, avatar]
        } else {
            row![avatar, bubble, Space::with_width(Length::Fill)]
        };

        container(row.spacing(10)).width(Length::Fill).into()
    }
}

fn collect_provider_options(config: &Config) -> Vec<String> {
    let mut providers = vec![
        "openai".to_string(),
        "anthropic".to_string(),
        "z.ai coding plan".to_string(),
        "ollama".to_string(),
        "openrouter".to_string(),
    ];

    for name in config.get_provider_names() {
        if !providers.iter().any(|p| p.eq_ignore_ascii_case(&name)) {
            providers.push(name);
        }
    }

    providers.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    providers
}

fn main() -> iced::Result {
    iced::application("Arula Desktop", App::update, App::view)
        .subscription(App::subscription)
        .theme(|_| app_theme())
        .run_with(App::init)
}
